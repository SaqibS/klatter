//! Pull rendering with exact source positions and utterance-local cancellation.

use std::ops::Range;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{AcousticPlan, Item, Pronunciations, Voice, klatt, text};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordEvent {
    pub item: usize,
    /// UTF-8 bytes within the original text item; expansions can share a range.
    pub text_range: Range<usize>,
    pub sample: usize,
}

#[derive(Clone, Default)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    pub samples: usize,
    /// Absolute sample positions, independent of the host's buffer size.
    pub marks: Vec<(u32, usize)>,
    pub words: Vec<WordEvent>,
}

enum Input {
    Text(String),
    Mark(u32),
}

pub struct Utterance {
    input: Vec<Input>,
    voice: Voice,
    pronunciations: Pronunciations,
    sample_rate: u32,
    synth: klatt::Synth,
    cancel: CancelHandle,
    plan: Option<AcousticPlan>,
    word_events: Vec<WordEvent>,
    next_word: usize,
    next_mark: usize,
    frame: usize,
    pending: Vec<i16>,
    pending_offset: usize,
    position: usize,
    done: bool,
}

impl Utterance {
    /// Owns the text and a snapshot of overrides; creating an utterance does no acoustic planning.
    pub fn new(
        items: &[Item],
        sample_rate: u32,
        voice: &Voice,
        pronunciations: &Pronunciations,
    ) -> Result<Self, &'static str> {
        if sample_rate == 0
            || !voice.wpm.is_finite()
            || voice.wpm <= 0.0
            || !voice.pitch_hz.is_finite()
            || voice.pitch_hz <= 0.0
            || !voice.inflection.is_finite()
            || voice.inflection < 0.0
            || !voice.volume.is_finite()
            || voice.volume < 0.0
        {
            return Err("invalid sample rate or voice settings");
        }
        Ok(Self {
            input: items
                .iter()
                .map(|item| match item {
                    Item::Text(text) => Input::Text((*text).to_owned()),
                    Item::Mark(id) => Input::Mark(*id),
                })
                .collect(),
            voice: *voice,
            pronunciations: pronunciations.clone(),
            sample_rate,
            synth: klatt::Synth::new(sample_rate),
            cancel: CancelHandle::default(),
            plan: None,
            word_events: Vec::new(),
            next_word: 0,
            next_mark: 0,
            frame: 0,
            pending: Vec::new(),
            pending_offset: 0,
            position: 0,
            done: false,
        })
    }

    pub fn cancel_handle(&self) -> CancelHandle {
        self.cancel.clone()
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn is_finished(&self) -> bool {
        self.done || self.cancel.is_cancelled()
    }

    fn plan(&mut self) {
        let mut tokens = Vec::new();
        let mut origins = Vec::new();
        for (item, input) in std::mem::take(&mut self.input).into_iter().enumerate() {
            if self.cancel.is_cancelled() {
                return;
            }
            match input {
                Input::Mark(id) => {
                    tokens.push(text::Token::Mark(id));
                }
                Input::Text(source) => {
                    if !text::append_with_spans_cancellable(
                        &source,
                        &mut tokens,
                        |token, range| {
                            origins.push((token, item, range));
                        },
                        &self.pronunciations,
                        &|| self.cancel.is_cancelled(),
                    ) {
                        return;
                    }
                }
            }
        }
        text::end_utterance(&mut tokens);
        let Some(plan) =
            crate::plan_tokens_cancellable(&tokens, &self.voice, &self.pronunciations, &|| {
                self.cancel.is_cancelled()
            })
        else {
            return;
        };
        let mut origins = origins.into_iter().peekable();
        self.word_events = plan
            .words
            .iter()
            .map(|&(token, frame)| {
                while origins.peek().is_some_and(|(index, _, _)| *index < token) {
                    origins.next();
                }
                let (index, item, range) = origins.next().expect("word source");
                assert_eq!(index, token);
                WordEvent {
                    item,
                    text_range: range,
                    sample: klatt::Synth::frame_start(frame, self.sample_rate),
                }
            })
            .collect();
        self.plan = Some(plan);
    }

    fn events(&mut self, rendered: &mut Rendered) {
        if let Some(plan) = &self.plan {
            while let Some(&(id, frame)) = plan.marks.get(self.next_mark) {
                let sample = klatt::Synth::frame_start(frame, self.sample_rate);
                if sample > self.position {
                    break;
                }
                rendered.marks.push((id, sample));
                self.next_mark += 1;
            }
        }
        while let Some(word) = self.word_events.get(self.next_word) {
            if word.sample > self.position {
                break;
            }
            rendered.words.push(word.clone());
            self.next_word += 1;
        }
    }

    /// Zeroes unused output. Empty buffers do not advance. Cancellation discards this
    /// call's output and events; audio already returned belongs to the host.
    pub fn render(&mut self, output: &mut [i16]) -> Rendered {
        output.fill(0);
        let start = self.position;
        let mut rendered = Rendered::default();
        if output.is_empty() {
            return rendered;
        }
        while rendered.samples < output.len() && !self.done {
            if self.cancel.is_cancelled() {
                output.fill(0);
                self.position = start;
                self.done = true;
                return Rendered::default();
            }
            if self.plan.is_none() {
                self.plan();
                if self.cancel.is_cancelled() {
                    continue;
                }
            }
            self.events(&mut rendered);
            if self.pending_offset == self.pending.len() {
                let plan = self.plan.as_ref().expect("acoustic plan");
                if self.frame == plan.frames.len() {
                    self.plan = None;
                    self.done = true;
                    break;
                }
                self.pending = self
                    .synth
                    .render(&plan.frames[self.frame..self.frame + 1])
                    .into_iter()
                    .map(|x| {
                        (x * crate::GAIN * self.voice.volume)
                            .round()
                            .clamp(i16::MIN as f64, i16::MAX as f64) as i16
                    })
                    .collect();
                self.frame += 1;
                self.pending_offset = 0;
                if self.pending.is_empty() {
                    continue;
                }
            }
            output[rendered.samples] = self.pending[self.pending_offset];
            self.pending_offset += 1;
            rendered.samples += 1;
            self.position += 1;
        }
        self.events(&mut rendered);
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(items: &[Item], rate: u32, size: usize) -> (Vec<i16>, Rendered) {
        let mut stream =
            Utterance::new(items, rate, &Voice::default(), &Pronunciations::default()).unwrap();
        let mut samples = Vec::new();
        let mut events = Rendered::default();
        loop {
            let mut buffer = vec![123; size];
            let block = stream.render(&mut buffer);
            samples.extend_from_slice(&buffer[..block.samples]);
            assert!(buffer[block.samples..].iter().all(|&x| x == 0));
            events.marks.extend(block.marks);
            events.words.extend(block.words);
            if stream.is_finished() {
                break;
            }
        }
        (samples, events)
    }

    #[test]
    fn pull_preserves_pcm_marks_and_utf8_sources_for_all_buffer_sizes() {
        let text = "Don\u{2019}t pay $12.50, NVDA!";
        let items = [
            Item::Mark(u32::MAX),
            Item::Text(text),
            Item::Mark(7),
            Item::Text("Dr. Smith?"),
            Item::Mark(7),
        ];
        for rate in [8000, 16001, 22050, 44100] {
            let expected = crate::speak(&items, rate, &Voice::default());
            let (reference, events) = collect(&items, rate, 137);
            assert_eq!(reference, expected.samples);
            assert_eq!(events.marks, expected.marks);
            assert_eq!(&text[events.words[0].text_range.clone()], "Don\u{2019}t");
            assert!(
                events
                    .words
                    .iter()
                    .filter(|w| w.item == 1)
                    .any(|w| &text[w.text_range.clone()] == "$12.50")
            );
            for size in [1, 4096] {
                assert_eq!(
                    collect(&items, rate, size),
                    (reference.clone(), events.clone())
                );
            }
        }
    }

    #[test]
    fn pull_planning_is_lazy_and_cancellation_is_per_utterance() {
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(30);
        let items = [Item::Text(&text), Item::Mark(9)];
        let (samples, events) = collect(&items, 22050, 137);
        assert_eq!(
            samples,
            crate::speak(&items, 22050, &Voice::default()).samples
        );
        assert_eq!(collect(&items, 22050, 4096), (samples, events));
        let mut stream =
            Utterance::new(&items, 16000, &Voice::default(), &Pronunciations::default()).unwrap();
        assert!(stream.plan.is_none());
        stream.render(&mut [0; 1]);
        assert!(stream.plan.is_some());
        let cancel = stream.cancel_handle();
        std::thread::spawn(move || cancel.cancel()).join().unwrap();
        let mut buffer = [123; 32];
        assert_eq!(stream.render(&mut buffer), Rendered::default());
        assert_eq!(buffer, [0; 32]);
        assert!(stream.is_finished());
        let mut fresh =
            Utterance::new(&items, 16000, &Voice::default(), &Pronunciations::default()).unwrap();
        assert_eq!(fresh.render(&mut buffer).samples, 32);
    }

    #[test]
    fn marks_without_audio_and_preplanning_cancellation_are_preserved() {
        let items = [Item::Mark(1), Item::Text(""), Item::Mark(2)];
        let (samples, rendered) = collect(&items, 16000, 13);
        assert!(samples.is_empty());
        assert_eq!(rendered.marks, [(1, 0), (2, 0)]);
        let mut stream = Utterance::new(
            &[Item::Text("Hello.")],
            16000,
            &Voice::default(),
            &Pronunciations::default(),
        )
        .unwrap();
        stream.cancel_handle().cancel();
        assert_eq!(stream.render(&mut [123; 32]), Rendered::default());
        assert!(stream.plan.is_none());
    }

    #[test]
    fn mixed_case_expansions_preserve_pcm_marks_and_source_events() {
        let source = "Caf\u{e9}Parser getXMLValue2";
        let items = [Item::Mark(1), Item::Text(source), Item::Mark(2)];
        let (samples, events) = collect(&items, 16001, 7);
        assert_eq!(
            samples,
            crate::speak(&items, 16001, &Voice::default()).samples
        );
        assert_eq!(collect(&items, 16001, 4096), (samples, events.clone()));
        assert_eq!(events.words.len(), 8);
        assert!(
            events.words[..2]
                .iter()
                .all(|w| &source[w.text_range.clone()] == "Caf\u{e9}Parser")
        );
        assert!(
            events.words[2..]
                .iter()
                .all(|w| &source[w.text_range.clone()] == "getXMLValue2")
        );
        assert_eq!(events.marks[0], (1, 0));
        assert!(events.marks[1].1 > events.words.last().unwrap().sample);
    }

    #[test]
    fn word_events_match_manual_word_boundaries_and_empty_reads_do_not_advance() {
        let items = [Item::Text("Hello"), Item::Mark(42), Item::Text("there.")];
        let mut stream =
            Utterance::new(&items, 16001, &Voice::default(), &Pronunciations::default()).unwrap();
        assert_eq!(stream.render(&mut []), Rendered::default());
        assert!(stream.plan.is_none());
        let (_, events) = collect(&items, 16001, 7);
        assert_eq!(events.words[1].item, 2);
        assert_eq!(events.words[1].text_range, 0..5);
        assert_eq!(events.words[1].sample, events.marks[0].1);
    }
}
