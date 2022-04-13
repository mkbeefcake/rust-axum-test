use axum::{
  body::Bytes,
  extract::{ContentLengthLimit, Multipart, TypedHeader, 
    ws::{
      WebSocket, WebSocketUpgrade, Message
    }
  },
  response::{Html, Redirect, IntoResponse}, 
  routing::get, 
  Router, BoxError,
  http::StatusCode,
};
use futures::{Stream, TryStreamExt};
use std::{io, net::SocketAddr};
use tokio::{fs::File, io::BufWriter};
use tokio_util::io::StreamReader;

#[tokio::main]
async fn main() {

  let app = Router::new()
    .route("/", get(handler).post(upload_file))
    .route("/ws", get(ws_handler));

  // run it
  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
  println!("Listening on {}", addr);

  axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await
    .unwrap();
}

async fn handler() -> Html<&'static str> {
  Html(r#"
    <html>
      <head></head>
      <body>
        <form action="/" method="post" enctype="multipart/form-data">
          <label> Upload file: <input type="file" name="file"></label>
          <input type="submit" value="upload">
          <input type="button" value="download">
        </form>
      </body>
    </html>
  "#)
}

async fn upload_file(
  ContentLengthLimit(mut multipart) : ContentLengthLimit<Multipart, { 250 * 1024 * 1024 }>
) -> Result<Redirect, (StatusCode, String)>
{
  while let Some(field) = multipart.next_field().await.unwrap() {
    let name = field.name().unwrap().to_string();
    let file_name = field.file_name().unwrap().to_string();
    let content_type = field.content_type().unwrap().to_string();
    
    println!(
      "uploading `{}`, `{}`, `{}`",
      name, file_name, content_type
    );

    stream_to_file(&file_name, field)
      .await
      .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
  }

  Ok(Redirect::to("/"))
}

// Save a `Stream` to a file
async fn stream_to_file<S, E>(path: &str, stream: S) -> Result<(), io::Error>
where
    S: Stream<Item = Result<Bytes, E>>,
    E: Into<BoxError>,
{
    // Convert the stream into an `AsyncRead`.
    let body_with_io_error = stream.map_err(|err| io::Error::new(io::ErrorKind::Other, err));
    let body_reader = StreamReader::new(body_with_io_error);
    futures::pin_mut!(body_reader);

    // Create the file. `File` implements `AsyncWrite`.
    let mut file = BufWriter::new(File::create(path).await?);

    // Copy the body into the file.
    tokio::io::copy(&mut body_reader, &mut file).await?;

    Ok(())
}

async fn ws_handler(
  ws:WebSocketUpgrade,
  user_agent: Option<TypedHeader<headers::UserAgent>>
) -> impl IntoResponse {
  if let Some(TypedHeader(user_agent)) = user_agent {
    println!("`{}` connected", user_agent.as_str());
  }

  ws.on_upgrade(handle_socket)
}

static mut PROGRESS: i32 = 0;

async fn handle_socket(mut socket: WebSocket) {

  loop {

    if let Some(msg) = socket.recv().await {
      if let Ok(msg) = msg {

        let request_msg = msg.into_text().unwrap();
        println!("Client updates the progress {:?}", request_msg);

        match request_msg.parse::<i32>() {
          Ok(n) => unsafe { 
            PROGRESS = n ;
          }
          Err(_) => {
          }
        }

        if socket
          .send(Message::Text(unsafe{ PROGRESS.to_string() } ))
          .await
          .is_err()
          {
            println!("client disconnected");
            return;
          }
      }
      else {
        print!("client disconnected");
        return;
      }
    }
  }

}