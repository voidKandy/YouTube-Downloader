use std::{path::PathBuf, sync::mpsc};

use anyhow::anyhow;
use rustube::{
    self, url::Url, video_info::player_response::streaming_data::QualityLabel, Callback, Stream,
    Video,
};
use std::sync::mpsc::{Receiver, Sender};

use eframe::egui;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    let (inner_sender, outer_receiver) = mpsc::channel::<Message>();
    let (outer_sender, inner_receiver) = mpsc::channel::<Message>();

    tokio::task::spawn(async move {
        if let Ok(Message::Download(url)) = outer_receiver.recv() {
            match download_video(&url).await {
                Ok(_) => outer_sender.send(Message::Success).unwrap(),
                Err(e) => outer_sender
                    .send(Message::Failure(format!(
                        "Failure downloading video: {:?}",
                        e
                    )))
                    .unwrap(),
            }
        }
    });
    let app = MyApp::from((inner_sender, inner_receiver));
    println!("App constructed");

    eframe::run_native("Youtube Downloader", options, Box::new(|_| Box::new(app)))
}

enum Message {
    Download(String),
    Success,
    Failure(String),
}

impl Into<String> for Message {
    fn into(self) -> String {
        match self {
            Message::Download(s) => s,
            Message::Failure(s) => s,
            Message::Success => "Success!".to_string(),
        }
    }
}

struct MyApp {
    url: String,
    status_message: String,
    sender: Sender<Message>,
    receiver: Receiver<Message>,
}

impl MyApp {
    fn update_status(&mut self) {
        match self.receiver.try_recv().ok() {
            Some(message) => {
                self.status_message = message.into();
            }
            None => {}
        }
    }
}

impl From<(Sender<Message>, Receiver<Message>)> for MyApp {
    fn from((sender, receiver): (Sender<Message>, Receiver<Message>)) -> Self {
        Self {
            url: String::new(),
            status_message: String::new(),
            sender,
            receiver,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let url_label = ui.label("Video URL: ");
                ui.text_edit_singleline(&mut self.url)
                    .labelled_by(url_label.id);
            });
            let download_button = ui.button("Download");
            let status = &self.status_message;
            ui.label(status);
            if download_button.clicked() {
                self.sender
                    .send(Message::Download(self.url.clone()))
                    .unwrap();
                self.status_message = String::new();
            }
            self.update_status();
        });
    }
}

async fn download_video(url: &str) -> Result<(), anyhow::Error> {
    let url = Url::parse(url)?;
    println!("Downloading video at URL: {}", url);
    let video = Video::from_url(&url).await?;
    let title = video.title().to_string().clone();

    println!("Title:  {}", title);

    let callback =
        Callback::new().connect_on_progress_closure(|args: rustube::CallbackArguments| {
            if let Some(len) = args.content_length {
                let percent = (args.current_chunk as f64 / len as f64) * 100.0;
                println!("{}%", percent.round());
            }
        });
    let acceptable_qualities = vec![
        QualityLabel::P720,
        QualityLabel::P720Hz50,
        QualityLabel::P720Hz60,
        QualityLabel::P1080,
        QualityLabel::P1080Hz50,
        QualityLabel::P1080Hz60,
    ];
    if let Some(stream) = video
        .into_streams()
        .into_iter()
        .filter(|s| {
            s.includes_video_track
                && s.includes_audio_track
                && acceptable_qualities.contains(&s.quality_label.unwrap_or(QualityLabel::P144))
        })
        .max_by_key(|s| s.quality_label)
    {
        println!(
            "Downloading video with quality: {:?}",
            stream.quality_label.unwrap()
        );
        let mut path = dirs::desktop_dir().unwrap();
        path.push(format!("{}.mp4", title));

        stream.download_to_with_callback(path, callback).await?;
        return Ok(());
    }
    Err(anyhow!("No stream with acceptable quality available"))
}
