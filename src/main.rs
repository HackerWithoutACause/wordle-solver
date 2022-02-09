use std::borrow::Cow;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::convert::TryInto;
use rayon::prelude::*;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use reedline::{Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal};

#[derive(PartialEq, Eq, Clone, Copy)]
struct Word([char; 5]);

impl std::fmt::Display for Word {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}{}{}{}{}", self.0[0], self.0[1], self.0[2], self.0[3], self.0[4])
    }
}

impl std::fmt::Debug for Word {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}{}{}{}{}", self.0[0], self.0[1], self.0[2], self.0[3], self.0[4])
    }
}

impl From<String> for Word {
    fn from(a: String) -> Self {
        Word(a.chars().collect::<Vec<char>>().try_into().unwrap())
    }
}

impl From<&str> for Word {
    fn from(a: &str) -> Self {
        Word(a.chars().collect::<Vec<char>>().try_into().unwrap())
    }
}

#[derive(Clone, Copy)]
enum Status {
    Exact,
    Found,
    None,
}

#[derive(Clone, Copy)]
struct Match {
    word: Word,
    status: [Status; 5],
}

fn find_in_word(word: Word, needle: char) -> Option<usize> {
    for i in 0..5 {
        if word.0[i] == needle {
            return Some(i);
        }
    }

    return None;
}

impl Match {
    fn new(word: Word) -> Self {
        Match {
            word,
            status: [Status::None; 5]
        }
    }

    fn mask(res: &str) -> [Status; 5] {
        let mut mat = Match::new(Word::from("panic"));

        for i in 0..5 {
            match res.chars().nth(i).unwrap() {
                '=' => mat.status[i] = Status::Exact,
                '~' => mat.status[i] = Status::Found,
                '.' => mat.status[i] = Status::None,
                _ => panic!("Unexpected character"),
            }
        }

        mat.status
    }

    fn input(word: Word, status: [Status; 5]) -> Self {
        Match {
            word,
            status,
        }
    }

    fn compute(guess: Word, mut ans: Word) -> Self {
        let mut mat = Match::new(guess);

        for i in 0..5 {
            if guess.0[i] == ans.0[i] {
                mat.status[i] = Status::Exact;
                ans.0[i] = '.';
            }
        }

        for i in 0..5 {
            if let Some(index) = find_in_word(ans, guess.0[i]) {
                mat.status[i] = Status::Found;
                ans.0[index] = '.';
            }
        }

        mat
    }

    fn valid(&self, mut word: Word) -> bool {
        for i in 0..5 {
            match self.status[i] {
                Status::Exact => {
                    if word.0[i] != self.word.0[i] {
                        return false;
                    } else {
                        word.0[i] = '.';
                    }
                }
                Status::Found => {
                    if word.0[i] == self.word.0[i] {
                        return false;
                    }
                }
                _ => ()
            }
        }

        for i in 0..5 {
            match self.status[i] {
                Status::Found => {
                    if let Some(index) = find_in_word(word, self.word.0[i]) {
                        word.0[index] = '.';
                    } else {
                        return false;
                    }
                }
                _ => ()
            }
        }

        for i in 0..5 {
            match self.status[i] {
                Status::None => {
                    if find_in_word(word, self.word.0[i]).is_some() {
                        return false;
                    }
                }
                _ => ()
            }
        }

        true
    }
}

impl std::fmt::Display for Match {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for i in 0..5 {
            match self.status[i] {
                Status::Exact => write!(f, "\x1B[32m{}\x1B[0m", self.word.0[i])?,
                Status::Found => write!(f, "\x1B[33m{}\x1B[0m", self.word.0[i])?,
                Status::None => write!(f, "{}", self.word.0[i])?,
            }
        }

        Ok(())
    }
}

fn score(guess: Word, words: &Vec<Word>) -> usize {
    words.par_iter()
        .map(|ans| {
            let mat = Match::compute(guess, *ans);
            found(mat, words)
        })
        .sum::<usize>()
}

// fn score_debug(guess: Word, words: &Vec<Word>) -> usize {
//     println!("\x1B[1m{}\x1B[0m", guess);

//     words.iter()
//         .map(|ans| {
//             let mat = Match::compute(guess, *ans);
//             let found = found(mat, words);
//             println!("{} = {} of {}", mat, found, ans);
//             found
//         })
//         .sum::<usize>()
// }

fn found(res: Match, words: &Vec<Word>) -> usize {
    let mut sum = 0;

    for word in words {
        if res.valid(*word) {
            sum += 1;
        }
    }

    sum
}

fn filter(res: Match, words: &mut Vec<Word>) {
    words.retain(|x| res.valid(*x))
        // .into_iter()
        // .par_iter()
        // .map(|x| *x)
        // .filter(|x| res.valid(*x))
        // .collect()
}

fn read_lines<P>(filename: P) -> io::Result<Vec<Word>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(
        io::BufReader::new(file)
            .lines()
            .map(Result::unwrap)
            .map(Word::from)
            .collect())
}

fn best_word(full_words: &Vec<Word>, ans: &Vec<Word>) -> Word {
    let bar = ProgressBar::new(full_words.len() as u64)
        .with_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .progress_chars("##-"));

    full_words.par_iter()
        .progress_with(bar)
        .map(|word| (*word, score(*word, &ans)))
        // .inspect(|x| println!("{} = {}", as_string(&x.0), x.1))
        .min_by(|a, b| a.1.cmp(&b.1))
        .unwrap().0
}

struct EmptyPrompt;

impl Prompt for EmptyPrompt {
    fn render_prompt(&self, _: usize) -> Cow<'_, str> {
        Cow::from("> ")
    }

    fn render_prompt_indicator(&self, _prompt_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::from("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::from("")
    }
    
    fn render_prompt_history_search_indicator(
        &self, 
        _history_search: PromptHistorySearch
    ) -> Cow<'_, str> {
        Cow::from("")
    }
}

fn main() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(8)
        .build_global()
        .unwrap();

    let mut answers = read_lines("answer_words.txt").unwrap();
    let full_words = read_lines("wordle.txt").unwrap();

    // let total = answers.par_iter()
    //     // .progress_count(answers.len() as u64)
    //     .map(|word| (word, simulate(*word, &full_words, &answers)))
    //     .inspect(|(word, count)| {
    //         if *count > 6 {
    //             println!("{} => \x1B[1;31m{}\x1B[0m", word, count)
    //         } else {
    //             println!("{} => {}", word, count)
    //         }
    //     })
    //     .map(|(_, count)| count)
    //     .sum::<u64>();

    // println!("Average words taken: {}", total as f64 / answers.len() as f64);

    let mut line_editor = Reedline::create().unwrap();
    let prompt = EmptyPrompt;

    guesser(&full_words, &answers, move |word| {
        print!("< {}", word);
        let input = line_editor.read_line(&prompt).unwrap();

        match input {
            Signal::Success(buffer) => Match::mask(&buffer),
            _ => panic!("Exiting"),
        }
    });
}

fn simulate(true_ans: Word, full_words: &Vec<Word>, answers: &Vec<Word>) -> u64 {
    guesser(full_words, answers, move |word| Match::compute(word, true_ans).status) as u64
}

fn guesser(full_words: &Vec<Word>, answers: &Vec<Word>, mut program: impl FnMut(Word) -> [Status; 5]) -> usize {
    let mut answers = answers.clone();
    let mut last_word = Word::from("roate");
    // let mut last_word = best_word(&answers, &answers);
    let mut guesses = 0;

    loop {
        // print!("< {}", last_word);
        // let input: String = read!("{}");
        // let input = line_editor.read_line(&prompt).unwrap();

        guesses += 1;
        let mask = program(last_word);
        filter(Match::input(last_word, mask), &mut answers);

        match mask {
            [Status::Exact, Status::Exact, Status::Exact, Status::Exact, Status::Exact] => break guesses,
            _ => (),
        }

        // match input {
        //     // "exit" => break,
        //     // "invalid" => words.retain(|&x| x != last_word),
        //     Signal::Success(buffer) => filter(Match::input(last_word, &buffer), &mut answers),
        //     Signal::CtrlL => line_editor.clear_screen().unwrap(),
        //     Signal::CtrlD | Signal::CtrlC => {
        //         println!("Exiting...");
        //         break
        //     },
        // }

        // println!("{:?}", answers);

        match answers.len() {
            // 0 => panic!("No possible words left"),
            0 => answers = full_words.clone(),
            1 | 2 => last_word = answers[0],
            _ => last_word = best_word(&full_words, &answers),
        }
    }
}
