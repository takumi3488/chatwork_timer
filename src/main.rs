use chrono::SubsecRound;
use log::{debug, error, info};
use serde::Deserialize;
use std::{env, process::exit};
use tokio::{
    fs::{remove_file, File, OpenOptions},
    io::{AsyncBufReadExt, AsyncWriteExt},
    time::{sleep, Duration},
};

const MESSAGE_ID_LOG: &str = "message-id.log";

#[tokio::main]
async fn main() {
    env_logger::init();
    File::create(MESSAGE_ID_LOG).await.unwrap();

    // 環境変数のチェック
    let token = env::var("CHATWORK_API_TOKEN").expect("CHATWORK_API_TOKEN is not set");
    let room_id = env::var("CHATWORK_ROOM_ID").expect("CHATWORK_ROOM_ID is not set");
    let working_minutes = env::var("WORKING_MINUTES")
        .map(|s| s.parse::<u32>().expect("WORKING_MINUTES is not a number"))
        .unwrap_or(25);
    let resting_minutes = env::var("RESTING_MINUTES")
        .map(|s| s.parse::<u32>().expect("RESTING_MINUTES is not a number"))
        .unwrap_or(5);
    if working_minutes < 1 || resting_minutes < 1 {
        panic!("WORKING_MINUTES and RESTING_MINUTES must be greater than 0");
    }
    let message_on_start_working =
        env::var("MESSAGE_ON_START_WORKING").unwrap_or("Working time! ~%time%".to_string());
    let message_on_start_resting =
        env::var("MESSAGE_ON_START_RESTING").unwrap_or("Resting time! ~%time%".to_string());

    // 初期状態の設定
    let mut is_working = false;

    // ctrl+cで終了するためのハンドラ
    let token_for_signal = token.clone();
    let room_id_for_signal = room_id.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        info!("Received SIGINT");
        let file = File::open(MESSAGE_ID_LOG)
            .await
            .expect("Failed to open message-id.log");
        let reader = tokio::io::BufReader::new(file);
        let mut lines = reader.lines();
        while let Some(line) = lines
            .next_line()
            .await
            .expect("Failed to read message-id.log")
        {
            let message_id = line.trim();
            match delete_message(&token_for_signal, &room_id_for_signal, message_id).await {
                Ok(_) => info!("Deleted message: {}", message_id),
                Err(e) => error!("{}", e),
            }
        }
        remove_file(MESSAGE_ID_LOG)
            .await
            .expect("Failed to remove message-id.log");
        info!("Exiting...");
        exit(0);
    });

    // ループ開始
    loop {
        // 状態遷移
        is_working = !is_working;

        // 現在時刻と丸めるための秒数を取得
        let now_second = chrono::Local::now().timestamp() % 60;
        let add_second = if now_second < 30 {
            -now_second
        } else {
            60 - now_second
        };
        let now = chrono::Local::now() + chrono::Duration::seconds(add_second);

        // 次に状態遷移する時刻を計算
        let change_state_time = if is_working {
            now + chrono::Duration::minutes(working_minutes as i64)
        } else {
            now + chrono::Duration::minutes(resting_minutes as i64)
        }
        .round_subsecs(0);

        // メッセージの作成
        let message = "[toall]\n".to_string()
            + &{
                if is_working {
                    message_on_start_working.clone()
                } else {
                    message_on_start_resting.clone()
                }
            }
            .replace("%time%", &change_state_time.format("%H:%M").to_string());
        debug!("Next state: {}", change_state_time);

        // spawn用に変数をクローン
        let token = token.clone();
        let room_id = room_id.clone();

        // メッセージの送信
        match send_message(&token, &room_id, &message).await {
            Ok(id) => {
                info!("{}", id);
                let mut file = OpenOptions::new()
                    .append(true)
                    .open(MESSAGE_ID_LOG)
                    .await
                    .expect("Failed to open message-id.log");
                file.write_all(format!("{}\n", id).as_bytes())
                    .await
                    .expect("Failed to write message-id.log");
            }
            Err(e) => {
                error!("{}", e);
                panic!("Failed to send message");
            }
        }

        // 次の状態遷移時刻まで待機
        let now = chrono::Local::now();
        let sleep_time = change_state_time - now;
        sleep(Duration::from_secs(sleep_time.num_seconds() as u64)).await;
    }
}

#[derive(Deserialize, Debug)]
struct MessageSendResponse {
    message_id: String,
}

/// メッセージを送信する
///
/// # Arguments
/// token: ChatWork APIのトークン
/// room_id: メッセージを送信するルームのID
/// message: 送信するメッセージ
/// # Returns
/// 成功時: メッセージID
/// 失敗時: エラーメッセージ
async fn send_message(token: &str, room_id: &str, message: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let res = client
        .post(&format!(
            "https://api.chatwork.com/v2/rooms/{}/messages",
            room_id
        ))
        .header("X-ChatWorkToken", token)
        .header("Accept", "application/json")
        .form(&[("body", message), ("self_unread", "1")])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    match res.json::<MessageSendResponse>().await {
        Ok(msg) => Ok(msg.message_id),
        Err(e) => Err(e.to_string()),
    }
}

/// メッセージを削除する
/// # Arguments
/// token: ChatWork APIのトークン
/// room_id: メッセージを送信するルームのID
/// message_id: 削除するメッセージのID
/// # Returns
/// 成功時: 空のResult
/// 失敗時: エラーメッセージ
async fn delete_message(token: &str, room_id: &str, message_id: &str) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client
        .delete(&format!(
            "https://api.chatwork.com/v2/rooms/{}/messages/{}",
            room_id, message_id
        ))
        .header("X-ChatWorkToken", token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("Failed to delete message: {}", res.status()))
    }
}
