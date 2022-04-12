use axum::{
  body::Bytes,
  extract::{ContentLengthLimit, Multipart},
  response::{Html, Redirect}, 
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

  let app = Router::new().route("/", get(handler).post(upload_file));

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