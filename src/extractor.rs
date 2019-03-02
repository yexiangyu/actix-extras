#![allow(dead_code)]
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::{fmt, str};

use bytes::Bytes;
use encoding::all::UTF_8;
use encoding::types::{DecoderTrap, Encoding};
use futures::future::{err, ok, Either, FutureResult};
use futures::{future, Async, Future, IntoFuture, Poll, Stream};
use mime::Mime;
use serde::de::{self, DeserializeOwned};
use serde::Serialize;
use serde_json;
use serde_urlencoded;

use actix_http::dev::{JsonBody, MessageBody, UrlEncoded};
use actix_http::error::{
    Error, ErrorBadRequest, ErrorNotFound, JsonPayloadError, PayloadError,
    UrlencodedError,
};
use actix_http::http::StatusCode;
use actix_http::{HttpMessage, Response};
use actix_router::PathDeserializer;

use crate::handler::FromRequest;
use crate::request::HttpRequest;
use crate::responder::Responder;
use crate::service::ServiceRequest;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
/// Extract typed information from the request's path.
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate bytes;
/// # extern crate actix_web;
/// # extern crate futures;
/// use actix_web::{http, App, Path, Result};
///
/// /// extract path info from "/{username}/{count}/index.html" url
/// /// {username} - deserializes to a String
/// /// {count} -  - deserializes to a u32
/// fn index(info: Path<(String, u32)>) -> Result<String> {
///     Ok(format!("Welcome {}! {}", info.0, info.1))
/// }
///
/// fn main() {
///     let app = App::new().resource(
///         "/{username}/{count}/index.html", // <- define path parameters
///         |r| r.method(http::Method::GET).with(index),
///     ); // <- use `with` extractor
/// }
/// ```
///
/// It is possible to extract path information to a specific type that
/// implements `Deserialize` trait from *serde*.
///
/// ```rust,ignore
/// # extern crate bytes;
/// # extern crate actix_web;
/// # extern crate futures;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{http, App, Path, Result};
///
/// #[derive(Deserialize)]
/// struct Info {
///     username: String,
/// }
///
/// /// extract path info using serde
/// fn index(info: Path<Info>) -> Result<String> {
///     Ok(format!("Welcome {}!", info.username))
/// }
///
/// fn main() {
///     let app = App::new().resource(
///         "/{username}/index.html", // <- define path parameters
///         |r| r.method(http::Method::GET).with(index),
///     ); // <- use `with` extractor
/// }
/// ```
pub struct Path<T> {
    inner: T,
}

impl<T> AsRef<T> for Path<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> Deref for Path<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for Path<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Path<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Extract path information from a request
    pub fn extract<P>(req: &ServiceRequest<P>) -> Result<Path<T>, de::value::Error>
    where
        T: DeserializeOwned,
    {
        de::Deserialize::deserialize(PathDeserializer::new(req.match_info()))
            .map(|inner| Path { inner })
    }
}

impl<T> From<T> for Path<T> {
    fn from(inner: T) -> Path<T> {
        Path { inner }
    }
}

impl<T, P> FromRequest<P> for Path<T>
where
    T: DeserializeOwned,
{
    type Error = Error;
    type Future = FutureResult<Self, Error>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        Self::extract(req).map_err(ErrorNotFound).into_future()
    }
}

impl<T: fmt::Debug> fmt::Debug for Path<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T: fmt::Display> fmt::Display for Path<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
/// Extract typed information from from the request's query.
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate bytes;
/// # extern crate actix_web;
/// # extern crate futures;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{App, Query, http};
///
///
///#[derive(Debug, Deserialize)]
///pub enum ResponseType {
///    Token,
///    Code
///}
///
///#[derive(Deserialize)]
///pub struct AuthRequest {
///    id: u64,
///    response_type: ResponseType,
///}
///
/// // use `with` extractor for query info
/// // this handler get called only if request's query contains `username` field
/// // The correct request for this handler would be `/index.html?id=64&response_type=Code"`
/// fn index(info: Query<AuthRequest>) -> String {
///     format!("Authorization request for client with id={} and type={:?}!", info.id, info.response_type)
/// }
///
/// fn main() {
///     let app = App::new().resource(
///        "/index.html",
///        |r| r.method(http::Method::GET).with(index)); // <- use `with` extractor
/// }
/// ```
pub struct Query<T>(T);

impl<T> Deref for Query<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Query<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Query<T> {
    /// Deconstruct to a inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, P> FromRequest<P> for Query<T>
where
    T: de::DeserializeOwned,
{
    type Error = Error;
    type Future = FutureResult<Self, Error>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        serde_urlencoded::from_str::<T>(req.query_string())
            .map(|val| ok(Query(val)))
            .unwrap_or_else(|e| err(e.into()))
    }
}

impl<T: fmt::Debug> fmt::Debug for Query<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: fmt::Display> fmt::Display for Query<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
/// Extract typed information from the request's body.
///
/// To extract typed information from request's body, the type `T` must
/// implement the `Deserialize` trait from *serde*.
///
/// [**FormConfig**](dev/struct.FormConfig.html) allows to configure extraction
/// process.
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate actix_web;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{App, Form, Result};
///
/// #[derive(Deserialize)]
/// struct FormData {
///     username: String,
/// }
///
/// /// extract form data using serde
/// /// this handler get called only if content type is *x-www-form-urlencoded*
/// /// and content of the request could be deserialized to a `FormData` struct
/// fn index(form: Form<FormData>) -> Result<String> {
///     Ok(format!("Welcome {}!", form.username))
/// }
/// # fn main() {}
/// ```
pub struct Form<T>(pub T);

impl<T> Form<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Form<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Form<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T, P> FromRequest<P> for Form<T>
where
    T: DeserializeOwned + 'static,
    P: Stream<Item = Bytes, Error = PayloadError> + 'static,
{
    type Error = Error;
    type Future = Box<Future<Item = Self, Error = Error>>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        let cfg = FormConfig::default();

        let req2 = req.clone();
        let err = Rc::clone(&cfg.ehandler);
        Box::new(
            UrlEncoded::new(req)
                .limit(cfg.limit)
                .map_err(move |e| (*err)(e, &req2))
                .map(Form),
        )
    }
}

impl<T: fmt::Debug> fmt::Debug for Form<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: fmt::Display> fmt::Display for Form<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Form extractor configuration
///
/// ```rust,ignore
/// # extern crate actix_web;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{http, App, Form, Result};
///
/// #[derive(Deserialize)]
/// struct FormData {
///     username: String,
/// }
///
/// /// extract form data using serde.
/// /// custom configuration is used for this handler, max payload size is 4k
/// fn index(form: Form<FormData>) -> Result<String> {
///     Ok(format!("Welcome {}!", form.username))
/// }
///
/// fn main() {
///     let app = App::new().resource(
///         "/index.html",
///         |r| {
///             r.method(http::Method::GET)
///                 // register form handler and change form extractor configuration
///                 .with_config(index, |cfg| {cfg.0.limit(4096);})
///         },
///     );
/// }
/// ```
pub struct FormConfig {
    limit: usize,
    ehandler: Rc<Fn(UrlencodedError, &HttpRequest) -> Error>,
}

impl FormConfig {
    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(&mut self, limit: usize) -> &mut Self {
        self.limit = limit;
        self
    }

    /// Set custom error handler
    pub fn error_handler<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(UrlencodedError, &HttpRequest) -> Error + 'static,
    {
        self.ehandler = Rc::new(f);
        self
    }
}

impl Default for FormConfig {
    fn default() -> Self {
        FormConfig {
            limit: 262_144,
            ehandler: Rc::new(|e, _| e.into()),
        }
    }
}

/// Json helper
///
/// Json can be used for two different purpose. First is for json response
/// generation and second is for extracting typed information from request's
/// payload.
///
/// To extract typed information from request's body, the type `T` must
/// implement the `Deserialize` trait from *serde*.
///
/// [**JsonConfig**](dev/struct.JsonConfig.html) allows to configure extraction
/// process.
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate actix_web;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{App, Json, Result, http};
///
/// #[derive(Deserialize)]
/// struct Info {
///     username: String,
/// }
///
/// /// deserialize `Info` from request's body
/// fn index(info: Json<Info>) -> Result<String> {
///     Ok(format!("Welcome {}!", info.username))
/// }
///
/// fn main() {
///     let app = App::new().resource(
///        "/index.html",
///        |r| r.method(http::Method::POST).with(index));  // <- use `with` extractor
/// }
/// ```
///
/// The `Json` type allows you to respond with well-formed JSON data: simply
/// return a value of type Json<T> where T is the type of a structure
/// to serialize into *JSON*. The type `T` must implement the `Serialize`
/// trait from *serde*.
///
/// ```rust,ignore
/// # extern crate actix_web;
/// # #[macro_use] extern crate serde_derive;
/// # use actix_web::*;
/// #
/// #[derive(Serialize)]
/// struct MyObj {
///     name: String,
/// }
///
/// fn index(req: HttpRequest) -> Result<Json<MyObj>> {
///     Ok(Json(MyObj {
///         name: req.match_info().query("name")?,
///     }))
/// }
/// # fn main() {}
/// ```
pub struct Json<T>(pub T);

impl<T> Json<T> {
    /// Deconstruct to an inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> fmt::Debug for Json<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Json: {:?}", self.0)
    }
}

impl<T> fmt::Display for Json<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<T: Serialize> Responder for Json<T> {
    type Error = Error;
    type Future = FutureResult<Response, Error>;

    fn respond_to(self, _: &HttpRequest) -> Self::Future {
        let body = match serde_json::to_string(&self.0) {
            Ok(body) => body,
            Err(e) => return err(e.into()),
        };

        ok(Response::build(StatusCode::OK)
            .content_type("application/json")
            .body(body))
    }
}

impl<T, P> FromRequest<P> for Json<T>
where
    T: DeserializeOwned + 'static,
    P: Stream<Item = Bytes, Error = PayloadError> + 'static,
{
    type Error = Error;
    type Future = Box<Future<Item = Self, Error = Error>>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        let cfg = JsonConfig::default();

        let req2 = req.clone();
        let err = Rc::clone(&cfg.ehandler);
        Box::new(
            JsonBody::new(req)
                .limit(cfg.limit)
                .map_err(move |e| (*err)(e, &req2))
                .map(Json),
        )
    }
}

/// Json extractor configuration
///
/// ```rust,ignore
/// # extern crate actix_web;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{error, http, App, HttpResponse, Json, Result};
///
/// #[derive(Deserialize)]
/// struct Info {
///     username: String,
/// }
///
/// /// deserialize `Info` from request's body, max payload size is 4kb
/// fn index(info: Json<Info>) -> Result<String> {
///     Ok(format!("Welcome {}!", info.username))
/// }
///
/// fn main() {
///     let app = App::new().resource("/index.html", |r| {
///         r.method(http::Method::POST)
///               .with_config(index, |cfg| {
///                   cfg.0.limit(4096)   // <- change json extractor configuration
///                      .error_handler(|err, req| {  // <- create custom error response
///                          error::InternalError::from_response(
///                              err, HttpResponse::Conflict().finish()).into()
///                          });
///               })
///     });
/// }
/// ```
pub struct JsonConfig {
    limit: usize,
    ehandler: Rc<Fn(JsonPayloadError, &HttpRequest) -> Error>,
}

impl JsonConfig {
    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(&mut self, limit: usize) -> &mut Self {
        self.limit = limit;
        self
    }

    /// Set custom error handler
    pub fn error_handler<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(JsonPayloadError, &HttpRequest) -> Error + 'static,
    {
        self.ehandler = Rc::new(f);
        self
    }
}

impl Default for JsonConfig {
    fn default() -> Self {
        JsonConfig {
            limit: 262_144,
            ehandler: Rc::new(|e, _| e.into()),
        }
    }
}

/// Request payload extractor.
///
/// Loads request's payload and construct Bytes instance.
///
/// [**PayloadConfig**](dev/struct.PayloadConfig.html) allows to configure
/// extraction process.
///
/// ## Example
///
/// ```rust,ignore
/// extern crate bytes;
/// # extern crate actix_web;
/// use actix_web::{http, App, Result};
///
/// /// extract text data from request
/// fn index(body: bytes::Bytes) -> Result<String> {
///     Ok(format!("Body {:?}!", body))
/// }
///
/// fn main() {
///     let app = App::new()
///         .resource("/index.html", |r| r.method(http::Method::GET).with(index));
/// }
/// ```
impl<P> FromRequest<P> for Bytes
where
    P: Stream<Item = Bytes, Error = PayloadError> + 'static,
{
    type Error = Error;
    type Future =
        Either<Box<Future<Item = Bytes, Error = Error>>, FutureResult<Bytes, Error>>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        let cfg = PayloadConfig::default();

        if let Err(e) = cfg.check_mimetype(req) {
            return Either::B(err(e));
        }

        Either::A(Box::new(MessageBody::new(req).limit(cfg.limit).from_err()))
    }
}

/// Extract text information from the request's body.
///
/// Text extractor automatically decode body according to the request's charset.
///
/// [**PayloadConfig**](dev/struct.PayloadConfig.html) allows to configure
/// extraction process.
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate actix_web;
/// use actix_web::{http, App, Result};
///
/// /// extract text data from request
/// fn index(body: String) -> Result<String> {
///     Ok(format!("Body {}!", body))
/// }
///
/// fn main() {
///     let app = App::new().resource("/index.html", |r| {
///         r.method(http::Method::GET)
///                .with_config(index, |cfg| { // <- register handler with extractor params
///                   cfg.0.limit(4096);  // <- limit size of the payload
///                 })
///     });
/// }
/// ```
impl<P> FromRequest<P> for String
where
    P: Stream<Item = Bytes, Error = PayloadError> + 'static,
{
    type Error = Error;
    type Future =
        Either<Box<Future<Item = String, Error = Error>>, FutureResult<String, Error>>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        let cfg = PayloadConfig::default();

        // check content-type
        if let Err(e) = cfg.check_mimetype(req) {
            return Either::B(err(e));
        }

        // check charset
        let encoding = match req.encoding() {
            Ok(enc) => enc,
            Err(e) => return Either::B(err(e.into())),
        };

        Either::A(Box::new(
            MessageBody::new(req)
                .limit(cfg.limit)
                .from_err()
                .and_then(move |body| {
                    let enc: *const Encoding = encoding as *const Encoding;
                    if enc == UTF_8 {
                        Ok(str::from_utf8(body.as_ref())
                            .map_err(|_| ErrorBadRequest("Can not decode body"))?
                            .to_owned())
                    } else {
                        Ok(encoding
                            .decode(&body, DecoderTrap::Strict)
                            .map_err(|_| ErrorBadRequest("Can not decode body"))?)
                    }
                }),
        ))
    }
}

/// Optionally extract a field from the request
///
/// If the FromRequest for T fails, return None rather than returning an error response
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate actix_web;
/// extern crate rand;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{http, App, Result, HttpRequest, Error, FromRequest};
/// use actix_web::error::ErrorBadRequest;
///
/// #[derive(Debug, Deserialize)]
/// struct Thing { name: String }
///
/// impl<S> FromRequest<S> for Thing {
///     type Config = ();
///     type Result = Result<Thing, Error>;
///
///     #[inline]
///     fn from_request(req: &HttpRequest<S>, _cfg: &Self::Config) -> Self::Result {
///         if rand::random() {
///             Ok(Thing { name: "thingy".into() })
///         } else {
///             Err(ErrorBadRequest("no luck"))
///         }
///
///     }
/// }
///
/// /// extract text data from request
/// fn index(supplied_thing: Option<Thing>) -> Result<String> {
///     match supplied_thing {
///         // Puns not intended
///         Some(thing) => Ok(format!("Got something: {:?}", thing)),
///         None => Ok(format!("No thing!"))
///     }
/// }
///
/// fn main() {
///     let app = App::new().resource("/users/:first", |r| {
///         r.method(http::Method::POST).with(index)
///     });
/// }
/// ```
impl<T: 'static, P> FromRequest<P> for Option<T>
where
    T: FromRequest<P>,
    T::Future: 'static,
{
    type Error = Error;
    type Future = Box<Future<Item = Option<T>, Error = Error>>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        Box::new(T::from_request(req).then(|r| match r {
            Ok(v) => future::ok(Some(v)),
            Err(_) => future::ok(None),
        }))
    }
}

/// Optionally extract a field from the request or extract the Error if unsuccessful
///
/// If the FromRequest for T fails, inject Err into handler rather than returning an error response
///
/// ## Example
///
/// ```rust,ignore
/// # extern crate actix_web;
/// extern crate rand;
/// #[macro_use] extern crate serde_derive;
/// use actix_web::{http, App, Result, HttpRequest, Error, FromRequest};
/// use actix_web::error::ErrorBadRequest;
///
/// #[derive(Debug, Deserialize)]
/// struct Thing { name: String }
///
/// impl FromRequest for Thing {
///     type Config = ();
///     type Result = Result<Thing, Error>;
///
///     #[inline]
///     fn from_request(req: &Request, _cfg: &Self::Config) -> Self::Result {
///         if rand::random() {
///             Ok(Thing { name: "thingy".into() })
///         } else {
///             Err(ErrorBadRequest("no luck"))
///         }
///
///     }
/// }
///
/// /// extract text data from request
/// fn index(supplied_thing: Result<Thing>) -> Result<String> {
///     match supplied_thing {
///         Ok(thing) => Ok(format!("Got thing: {:?}", thing)),
///         Err(e) => Ok(format!("Error extracting thing: {}", e))
///     }
/// }
///
/// fn main() {
///     let app = App::new().resource("/users/:first", |r| {
///         r.method(http::Method::POST).with(index)
///     });
/// }
/// ```
impl<T: 'static, P> FromRequest<P> for Result<T, T::Error>
where
    T: FromRequest<P>,
    T::Future: 'static,
    T::Error: 'static,
{
    type Error = Error;
    type Future = Box<Future<Item = Result<T, T::Error>, Error = Error>>;

    #[inline]
    fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
        Box::new(T::from_request(req).then(|res| match res {
            Ok(v) => ok(Ok(v)),
            Err(e) => ok(Err(e)),
        }))
    }
}

/// Payload configuration for request's payload.
pub struct PayloadConfig {
    limit: usize,
    mimetype: Option<Mime>,
}

impl PayloadConfig {
    /// Change max size of payload. By default max size is 256Kb
    pub fn limit(&mut self, limit: usize) -> &mut Self {
        self.limit = limit;
        self
    }

    /// Set required mime-type of the request. By default mime type is not
    /// enforced.
    pub fn mimetype(&mut self, mt: Mime) -> &mut Self {
        self.mimetype = Some(mt);
        self
    }

    fn check_mimetype<P>(&self, req: &ServiceRequest<P>) -> Result<(), Error> {
        // check content-type
        if let Some(ref mt) = self.mimetype {
            match req.mime_type() {
                Ok(Some(ref req_mt)) => {
                    if mt != req_mt {
                        return Err(ErrorBadRequest("Unexpected Content-Type"));
                    }
                }
                Ok(None) => {
                    return Err(ErrorBadRequest("Content-Type is expected"));
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }
        Ok(())
    }
}

impl Default for PayloadConfig {
    fn default() -> Self {
        PayloadConfig {
            limit: 262_144,
            mimetype: None,
        }
    }
}

macro_rules! tuple_from_req ({$fut_type:ident, $(($n:tt, $T:ident)),+} => {

    /// FromRequest implementation for tuple
    impl<P, $($T: FromRequest<P> + 'static),+> FromRequest<P> for ($($T,)+)
    {
        type Error = Error;
        type Future = $fut_type<P, $($T),+>;

        fn from_request(req: &mut ServiceRequest<P>) -> Self::Future {
            $fut_type {
                items: <($(Option<$T>,)+)>::default(),
                futs: ($($T::from_request(req),)+),
            }
        }
    }

    #[doc(hidden)]
    pub struct $fut_type<P, $($T: FromRequest<P>),+> {
        items: ($(Option<$T>,)+),
        futs: ($($T::Future,)+),
    }

    impl<P, $($T: FromRequest<P>),+> Future for $fut_type<P, $($T),+>
    {
        type Item = ($($T,)+);
        type Error = Error;

        fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
            let mut ready = true;

            $(
                if self.items.$n.is_none() {
                    match self.futs.$n.poll() {
                        Ok(Async::Ready(item)) => {
                            self.items.$n = Some(item);
                        }
                        Ok(Async::NotReady) => ready = false,
                        Err(e) => return Err(e.into()),
                    }
                }
            )+

                if ready {
                    Ok(Async::Ready(
                        ($(self.items.$n.take().unwrap(),)+)
                    ))
                } else {
                    Ok(Async::NotReady)
                }
        }
    }
});

impl<P> FromRequest<P> for () {
    type Error = Error;
    type Future = FutureResult<(), Error>;

    fn from_request(_req: &mut ServiceRequest<P>) -> Self::Future {
        ok(())
    }
}

tuple_from_req!(TupleFromRequest1, (0, A));
tuple_from_req!(TupleFromRequest2, (0, A), (1, B));
tuple_from_req!(TupleFromRequest3, (0, A), (1, B), (2, C));
tuple_from_req!(TupleFromRequest4, (0, A), (1, B), (2, C), (3, D));
tuple_from_req!(TupleFromRequest5, (0, A), (1, B), (2, C), (3, D), (4, E));
tuple_from_req!(
    TupleFromRequest6,
    (0, A),
    (1, B),
    (2, C),
    (3, D),
    (4, E),
    (5, F)
);
tuple_from_req!(
    TupleFromRequest7,
    (0, A),
    (1, B),
    (2, C),
    (3, D),
    (4, E),
    (5, F),
    (6, G)
);
tuple_from_req!(
    TupleFromRequest8,
    (0, A),
    (1, B),
    (2, C),
    (3, D),
    (4, E),
    (5, F),
    (6, G),
    (7, H)
);
tuple_from_req!(
    TupleFromRequest9,
    (0, A),
    (1, B),
    (2, C),
    (3, D),
    (4, E),
    (5, F),
    (6, G),
    (7, H),
    (8, I)
);
tuple_from_req!(
    TupleFromRequest10,
    (0, A),
    (1, B),
    (2, C),
    (3, D),
    (4, E),
    (5, F),
    (6, G),
    (7, H),
    (8, I),
    (9, J)
);

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use actix_http::http::header;
//     use actix_http::test::TestRequest;
//     use bytes::Bytes;
//     use futures::{Async, Future};
//     use mime;
//     use serde::{Deserialize, Serialize};

//     use crate::resource::Resource;
//     // use crate::router::{ResourceDef, Router};

//     #[derive(Deserialize, Debug, PartialEq)]
//     struct Info {
//         hello: String,
//     }

//     #[test]
//     fn test_bytes() {
//         let cfg = PayloadConfig::default();
//         let req = TestRequest::with_header(header::CONTENT_LENGTH, "11")
//             .set_payload(Bytes::from_static(b"hello=world"))
//             .finish();

//         match Bytes::from_request(&req, &cfg).unwrap().poll().unwrap() {
//             Async::Ready(s) => {
//                 assert_eq!(s, Bytes::from_static(b"hello=world"));
//             }
//             _ => unreachable!(),
//         }
//     }

//     #[test]
//     fn test_string() {
//         let cfg = PayloadConfig::default();
//         let req = TestRequest::with_header(header::CONTENT_LENGTH, "11")
//             .set_payload(Bytes::from_static(b"hello=world"))
//             .finish();

//         match String::from_request(&req, &cfg).unwrap().poll().unwrap() {
//             Async::Ready(s) => {
//                 assert_eq!(s, "hello=world");
//             }
//             _ => unreachable!(),
//         }
//     }

//     #[test]
//     fn test_form() {
//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .header(header::CONTENT_LENGTH, "11")
//         .set_payload(Bytes::from_static(b"hello=world"))
//         .finish();

//         let mut cfg = FormConfig::default();
//         cfg.limit(4096);
//         match Form::<Info>::from_request(&req, &cfg).poll().unwrap() {
//             Async::Ready(s) => {
//                 assert_eq!(s.hello, "world");
//             }
//             _ => unreachable!(),
//         }
//     }

//     #[test]
//     fn test_option() {
//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .finish();

//         let mut cfg = FormConfig::default();
//         cfg.limit(4096);

//         match Option::<Form<Info>>::from_request(&req, &cfg)
//             .poll()
//             .unwrap()
//         {
//             Async::Ready(r) => assert_eq!(r, None),
//             _ => unreachable!(),
//         }

//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .header(header::CONTENT_LENGTH, "9")
//         .set_payload(Bytes::from_static(b"hello=world"))
//         .finish();

//         match Option::<Form<Info>>::from_request(&req, &cfg)
//             .poll()
//             .unwrap()
//         {
//             Async::Ready(r) => assert_eq!(
//                 r,
//                 Some(Form(Info {
//                     hello: "world".into()
//                 }))
//             ),
//             _ => unreachable!(),
//         }

//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .header(header::CONTENT_LENGTH, "9")
//         .set_payload(Bytes::from_static(b"bye=world"))
//         .finish();

//         match Option::<Form<Info>>::from_request(&req, &cfg)
//             .poll()
//             .unwrap()
//         {
//             Async::Ready(r) => assert_eq!(r, None),
//             _ => unreachable!(),
//         }
//     }

//     #[test]
//     fn test_result() {
//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .header(header::CONTENT_LENGTH, "11")
//         .set_payload(Bytes::from_static(b"hello=world"))
//         .finish();

//         match Result::<Form<Info>, Error>::from_request(&req, &FormConfig::default())
//             .poll()
//             .unwrap()
//         {
//             Async::Ready(Ok(r)) => assert_eq!(
//                 r,
//                 Form(Info {
//                     hello: "world".into()
//                 })
//             ),
//             _ => unreachable!(),
//         }

//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .header(header::CONTENT_LENGTH, "9")
//         .set_payload(Bytes::from_static(b"bye=world"))
//         .finish();

//         match Result::<Form<Info>, Error>::from_request(&req, &FormConfig::default())
//             .poll()
//             .unwrap()
//         {
//             Async::Ready(r) => assert!(r.is_err()),
//             _ => unreachable!(),
//         }
//     }

//     #[test]
//     fn test_payload_config() {
//         let req = TestRequest::default().finish();
//         let mut cfg = PayloadConfig::default();
//         cfg.mimetype(mime::APPLICATION_JSON);
//         assert!(cfg.check_mimetype(&req).is_err());

//         let req = TestRequest::with_header(
//             header::CONTENT_TYPE,
//             "application/x-www-form-urlencoded",
//         )
//         .finish();
//         assert!(cfg.check_mimetype(&req).is_err());

//         let req =
//             TestRequest::with_header(header::CONTENT_TYPE, "application/json").finish();
//         assert!(cfg.check_mimetype(&req).is_ok());
//     }

//     #[derive(Deserialize)]
//     struct MyStruct {
//         key: String,
//         value: String,
//     }

//     #[derive(Deserialize)]
//     struct Id {
//         id: String,
//     }

//     #[derive(Deserialize)]
//     struct Test2 {
//         key: String,
//         value: u32,
//     }

//     #[test]
//     fn test_request_extract() {
//         let req = TestRequest::with_uri("/name/user1/?id=test").finish();

//         let mut router = Router::<()>::default();
//         router.register_resource(Resource::new(ResourceDef::new("/{key}/{value}/")));
//         let info = router.recognize(&req, &(), 0);
//         let req = req.with_route_info(info);

//         let s = Path::<MyStruct>::from_request(&req, &()).unwrap();
//         assert_eq!(s.key, "name");
//         assert_eq!(s.value, "user1");

//         let s = Path::<(String, String)>::from_request(&req, &()).unwrap();
//         assert_eq!(s.0, "name");
//         assert_eq!(s.1, "user1");

//         let s = Query::<Id>::from_request(&req, &()).unwrap();
//         assert_eq!(s.id, "test");

//         let mut router = Router::<()>::default();
//         router.register_resource(Resource::new(ResourceDef::new("/{key}/{value}/")));
//         let req = TestRequest::with_uri("/name/32/").finish();
//         let info = router.recognize(&req, &(), 0);
//         let req = req.with_route_info(info);

//         let s = Path::<Test2>::from_request(&req, &()).unwrap();
//         assert_eq!(s.as_ref().key, "name");
//         assert_eq!(s.value, 32);

//         let s = Path::<(String, u8)>::from_request(&req, &()).unwrap();
//         assert_eq!(s.0, "name");
//         assert_eq!(s.1, 32);

//         let res = Path::<Vec<String>>::extract(&req).unwrap();
//         assert_eq!(res[0], "name".to_owned());
//         assert_eq!(res[1], "32".to_owned());
//     }

//     #[test]
//     fn test_extract_path_single() {
//         let mut router = Router::<()>::default();
//         router.register_resource(Resource::new(ResourceDef::new("/{value}/")));

//         let req = TestRequest::with_uri("/32/").finish();
//         let info = router.recognize(&req, &(), 0);
//         let req = req.with_route_info(info);
//         assert_eq!(*Path::<i8>::from_request(&req, &()).unwrap(), 32);
//     }

//     #[test]
//     fn test_tuple_extract() {
//         let mut router = Router::<()>::default();
//         router.register_resource(Resource::new(ResourceDef::new("/{key}/{value}/")));

//         let req = TestRequest::with_uri("/name/user1/?id=test").finish();
//         let info = router.recognize(&req, &(), 0);
//         let req = req.with_route_info(info);

//         let res = match <(Path<(String, String)>,)>::extract(&req).poll() {
//             Ok(Async::Ready(res)) => res,
//             _ => panic!("error"),
//         };
//         assert_eq!((res.0).0, "name");
//         assert_eq!((res.0).1, "user1");

//         let res = match <(Path<(String, String)>, Path<(String, String)>)>::extract(&req)
//             .poll()
//         {
//             Ok(Async::Ready(res)) => res,
//             _ => panic!("error"),
//         };
//         assert_eq!((res.0).0, "name");
//         assert_eq!((res.0).1, "user1");
//         assert_eq!((res.1).0, "name");
//         assert_eq!((res.1).1, "user1");

//         let () = <()>::extract(&req);
//     }
// }
