use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use thiserror::Error;

#[derive(Clone)]
pub struct Tasks {
    id: usize,
    sender: flume::Sender<Messages>,
}

impl Drop for Tasks {
    fn drop(&mut self) {
        let _ = self.sender.send(Messages::Kill);
    }
}

pub struct TaskRef {
    id: usize,
    sender: flume::Sender<Messages>,
}

impl TaskRef {
    pub fn set_subtask(&self, subtask: &str) {
        let _ = self
            .sender
            .send(Messages::SetSubtask(self.id, subtask.into(), None));
    }

    pub fn set_subtask_with_percentage(&self, subtask: &str, p: f64) {
        let _ = self
            .sender
            .send(Messages::SetSubtask(self.id, subtask.into(), Some(p)));
    }

    /// Mark the task as done with a checkmark message.
    pub fn finish(&self) {
        let _ = self.sender.send(Messages::Finish(self.id, None));
    }

    /// Mark the task as done with a custom trailing message (e.g. "skipped, already done").
    pub fn finish_with_message(&self, msg: &str) {
        let _ = self.sender.send(Messages::Finish(self.id, Some(msg.into())));
    }

    pub fn set_percentage(&self, p: f64) {
        let _ = self
            .sender
            .send(Messages::SetPercentage(self.id, p.clamp(0.0, 1.0)));
    }
}

pub struct Task {
    name: String,
    subtask: Option<String>,
    pb: ProgressBar,
    width: usize,
}

impl Task {
    pub fn update(&self, i: usize, n: usize) {
        self.pb.set_prefix(format!("[{}/{}]", i + 1, n));

        let mut msg = match self.subtask.as_ref() {
            Some(subtask) => format!("{} - {}", self.name, subtask),
            None => self.name.clone(),
        };

        if msg.len() > self.width {
            // if not ascii we may have trouble with truncate
            if !msg.is_ascii() {
                msg = self.name.clone();
            } else {
                msg.truncate(self.width - 3);
                msg.push_str("...");
            }
        }

        self.pb.set_message(msg);
        self.pb.tick();
    }
}

pub enum Messages {
    NewTask { name: String },
    SetSubtask(usize, String, Option<f64>),
    Finish(usize, Option<String>),
    SetPercentage(usize, f64), // between 0 and 1
    Kill,
}

fn build_style(msg_width: usize) -> ProgressStyle {
    ProgressStyle::with_template(&format!(
        "{{prefix:.bold.dim}} {{spinner}} {{msg:{msg_width}.cyan}} {{bar:30.cyan/blue}} {{percent:>3}}% {{eta:.dim}}"
    ))
    .expect("progress template is valid")
    .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
    .progress_chars("█▓▒░ ")
}

async fn tick_progress_bars(r: flume::Receiver<Messages>) {
    let (w, _) = term_size::dimensions().unwrap_or((80, 0));
    // Reserve room for prefix(8) + spinner(1) + spaces + bar(30) + percent(4) + eta(8).
    let msg_width = w.saturating_sub(60).max(10);

    let m = MultiProgress::new();
    let style = build_style(msg_width);
    let mut tasks: Vec<Task> = vec![];

    loop {
        tokio::select! {
            msg = r.recv_async() => {
                match msg {
                    Ok(Messages::NewTask { name }) => {
                        let pb = m.add(ProgressBar::new(100));
                        pb.set_style(style.clone());
                        pb.enable_steady_tick(std::time::Duration::from_millis(120));

                        let t = Task { name, subtask: None, pb, width: msg_width };
                        tasks.push(t);

                        let n = tasks.len();
                        for (i, t) in tasks.iter().enumerate() {
                            t.update(i, n);
                        }
                    }
                    Ok(Messages::SetSubtask(i, subtask, p)) => {
                        let n = tasks.len();
                        if let Some(t) = tasks.get_mut(i) {
                            t.subtask = Some(subtask);
                            t.pb.set_position((p.unwrap_or_default() * 100.0) as u64);
                            t.update(i, n);
                        }
                    }
                    Ok(Messages::Finish(i, msg)) => {
                        if let Some(t) = tasks.get(i) {
                            t.pb.set_position(100);
                            let suffix = msg
                                .as_deref()
                                .map(|m| format!(" ✓ ({m})"))
                                .unwrap_or_else(|| " ✓".to_string());
                            t.pb.finish_with_message(format!("{}{}", t.name, suffix));
                        }
                    }
                    Ok(Messages::SetPercentage(i, p)) => {
                        if let Some(t) = tasks.get(i) {
                            t.pb.set_position((p * 100.0) as u64);
                        }
                    }
                    Ok(Messages::Kill) | Err(_) => break,
                }
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum TaskErrors {
    #[error("progress report is dead")]
    BackgroundTaskDead,
}

impl Tasks {
    pub fn new() -> Tasks {
        let (sender, r) = flume::unbounded();
        tokio::spawn(tick_progress_bars(r));
        Tasks { id: 0, sender }
    }

    pub fn new_task(&mut self, name: &str) -> Result<TaskRef, TaskErrors> {
        self.sender
            .send(Messages::NewTask { name: name.into() })
            .map_err(|_| TaskErrors::BackgroundTaskDead)?;

        let id = self.id;
        self.id += 1;

        Ok(TaskRef {
            id,
            sender: self.sender.clone(),
        })
    }
}