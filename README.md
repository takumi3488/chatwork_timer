# Chatwork Timer

作業と休憩を繰り返したい時に決まった間隔でTO ALL通知します。

送信したメッセージはプログラム停止時に削除されます。

通知のタイミングは分単位で丸められるので、数分程度の使用には向いていません。

## 環境変数

|環境変数名|デフォルト値|備考|
|---|---|---|
|CHATWORK_API_TOKEN||必須|
|CHATWORK_ROOM_ID||必須|
|WORKING_MINUTES|25||
|RESTING_MINUTES|5||
|MESSAGE_ON_START_WORK|Working time! ~%time%|`%time%`部分は終了時間(`HH:MM`)|
|MESSAGE_ON_START_REST|Resting time! ~%time%|`%time%`部分は終了時間(`HH:MM`)|
|RUST_LOG|error|ログレベル|
