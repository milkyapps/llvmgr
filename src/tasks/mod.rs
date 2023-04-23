use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use thiserror::Error;

#[derive(Clone)]
pub struct Tasks {
    id: usize,
    sender: flume::Sender<Messages>,
}

impl Drop for Tasks {
    fn drop(&mut self) {
        self.sender.send(Messages::Kill).unwrap();
    }
}

pub struct TaskRef {
    id: usize,
    sender: flume::Sender<Messages>,
}

impl TaskRef {
    pub fn set_subtask(&self, subtask: &str) {
        self.sender
            .send(Messages::SetSubtask(self.id, subtask.into(), None))
            .unwrap();
    }

    pub fn set_subtask_with_percentage(&self, subtask: &str, p: f64) {
        self.sender
            .send(Messages::SetSubtask(self.id, subtask.into(), Some(p)))
            .unwrap();
    }

    pub fn finish(&self) {
        self.sender.send(Messages::Finish(self.id)).unwrap();
    }

    pub fn set_percentage(&self, p: f64) {
        self.sender
            .send(Messages::SetPercentage(self.id, p))
            .unwrap();
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

        let mut msg = if let Some(subtask) = self.subtask.as_ref() {
            format!("{} - {}", self.name, subtask)
        } else {
            self.name.clone()
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
    Finish(usize),
    SetPercentage(usize, f64), // between 0 and 1,
    Kill,
}

async fn tick_progress_bars(r: flume::Receiver<Messages>) {
    let (w, _) = term_size::dimensions().unwrap_or((80, 0));

    let msg_width = w - 55;
    let template =
        format!("{{prefix:.bold.dim}} {{spinner}} {{msg:{msg_width}}} {{bar:40}} {{eta}}");

    let m = MultiProgress::new();
    let style = ProgressStyle::with_template(&template)
        .expect("should not fail")
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
    let mut tasks = vec![];

    loop {
        tokio::select! {
            msg = r.recv_async() => {
                match msg {
                    Ok(Messages::NewTask{ name }) => {
                        let pb = m.add(ProgressBar::new(100));
                        pb.set_style(style.clone());

                        let t= Task { name, subtask: None, pb, width: msg_width };
                        tasks.push(t);

                        let n = tasks.len();
                        for (i, t) in tasks.iter().enumerate() {
                            t.update(i, n);
                        }
                    }
                    Ok(Messages::SetSubtask(i, subtask, p)) => {
                        tasks[i].subtask = Some(subtask);
                        tasks[i].pb.set_position((p.unwrap_or_default() * 100.0) as u64);
                        tasks[i].update(i, tasks.len());
                    }
                    Ok(Messages::Finish(i)) => {
                        tasks[i].subtask = None;
                        tasks[i].pb.finish();
                        tasks[i].update(i, tasks.len());
                    }
                    Ok(Messages::SetPercentage(i, p)) => {
                        tasks[i].pb.set_position((p * 100.0) as u64);
                    }
                    Ok(Messages::Kill) | Err(_) => break
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(1000)) => {
                // let n = tasks.len();
                // for (i, t) in tasks.iter().enumerate() {
                //     t.update(i, n);
                // }
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
