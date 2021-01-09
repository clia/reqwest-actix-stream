use std::io;

use actix_web::web;

use std::pin::Pin;
use std::task::{Context, Poll};
use futures::Stream;
use actix_http::error::PayloadError;

/// 把Actix-web 的 Payload 强行改成可发送的
/// 这样才能支持reqwest的流分发方式
/// 
/// 注意：此物最好不要跨线程使用，否则就真的不安全了
/// 
/// ## Example
///
/// ```rust
/// async fn handle(
///     body: actix_web::web::Payload,
/// ) {
///     let mut builder = client.get(url);
///     builder = builder.body(reqwest::Body::wrap_stream(reqwest_actix_stream::PayloadStream {
///         payload: body,
///     }));
///     builder.send().await;
/// }
/// ```
pub struct PayloadStream {
    pub payload: web::Payload,
}

unsafe impl Send for PayloadStream {}
unsafe impl Sync for PayloadStream {}

impl Stream for PayloadStream {
    type Item = Result<web::Bytes, io::Error>;

    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {

        // 由于Actix-web 的 PayloadError 只在体系内，这里需要转换一下才能传给reqwest
        match Pin::new(&mut self.payload).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(res))) => Poll::Ready(Some(Ok(res))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(match e {
                PayloadError::Incomplete(o) => match o {
                    Some(e) => e,
                    None => io::Error::new(io::ErrorKind::Other, "PayloadError::Incomplete None"),
                },
                PayloadError::EncodingCorrupted => io::Error::new(io::ErrorKind::Other, "PayloadError::EncodingCorrupted"),
                PayloadError::Overflow => io::Error::new(io::ErrorKind::Other, "PayloadError::Overflow"),
                PayloadError::UnknownLength => io::Error::new(io::ErrorKind::Other, "PayloadError::UnknownLength"),
                PayloadError::Http2Payload(e) => io::Error::new(io::ErrorKind::Other, format!("PayloadError::Http2Payload {:?}", e)),
                PayloadError::Io(e) => e,
            }))),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}

/// 把 reqwest 的流也封装一下，转换需要的错误类型
/// 
/// ## Example
///
/// ```rust
/// let res = builder.send().await;
/// let stream = res.bytes_stream();
/// let mut resp = HttpResponse::build(res.status());
/// // 这种方式默认会使用 chunked 传输方式
/// return Ok(resp.streaming(reqwest_actix_stream::ResponseStream{ stream: stream }));
/// ```
pub struct ResponseStream<T> where T: Stream<Item = reqwest::Result<web::Bytes>> + Unpin {
    // stream: Box<dyn Stream<Item = reqwest::Result<web::Bytes>>>,
    // stream: Box<dyn Stream<Item = reqwest::Result<web::Bytes>>>,
    pub stream: T,
}

impl<T> Stream for ResponseStream<T> where T: Stream<Item = reqwest::Result<web::Bytes>> + Unpin {
    type Item = Result<web::Bytes, actix_web::Error>;

    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {

        // 由于Actix-web 和 reqwest 的错误体系不互通，这里需要转换一下
        // match Pin::new(&mut self.stream).poll_next(cx) {
        // match Pin::new(Box::leak(self.stream)).poll_next(cx) {
        // let s = Pin::new(&mut self.stream);
        // let s2 = unsafe { Box::from_raw(Box::into_raw(self.stream)) };
        // match <Pin<Box<_>>>::from(self.stream).as_mut().poll_next(cx) {
        // match s.as_mut().poll_next(cx) {
        // match Pin::new(&mut Box::new(&mut self.stream)).poll_next(cx) {
        // match Pin::new((&mut self.stream).as_mut()).poll_next(cx) {
        // match self.stream.poll_next(cx) {
        // match Box::pin(Box::new(self.stream)).as_mut().poll_next(cx) {
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(Ok(res))) => Poll::Ready(Some(Ok(res))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(io::Error::new(io::ErrorKind::Other, format!("{:?}", e)).into()))),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}
