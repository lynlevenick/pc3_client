extern crate cookie;
extern crate hyper;
extern crate rand;
// Freakin really? Why are dashes allowed here?
extern crate rustc_serialize as serialize;

use cookie::CookieJar;
use hyper::{Client, header};
use hyper::header::Headers;
use rand::Rng;
use serialize::json;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;

struct Pc3Client {
    url: String,
    jar: CookieJar<'static>
}

#[derive(Debug)]
enum Pc3Error {
    Http(hyper::Error),
    Json(json::DecoderError),
    Io(io::Error),
    Other(&'static str)
}
impl From<hyper::Error> for Pc3Error {
    fn from(err: hyper::Error) -> Pc3Error {
        Pc3Error::Http(err)
    }
}
impl From<json::DecoderError> for Pc3Error {
    fn from(err: json::DecoderError) -> Pc3Error {
        Pc3Error::Json(err)
    }
}
impl From<io::Error> for Pc3Error {
    fn from(err: io::Error) -> Pc3Error {
        Pc3Error::Io(err)
    }
}

fn create_submit_body(boundary: &str, src: &mut File, src_name: &str) -> Result<Vec<u8>, Pc3Error> {
    let mut file_content = Vec::new();
    try!(src.read_to_end(&mut file_content));

    // There has to be a better way
    Ok("--".bytes()
       .chain(boundary.bytes())
       .chain("\nContent-Disposition: form-data; name=\"teamCode\"; filename=\"".bytes())
       .chain(file_name(src_name).bytes())
       .chain("\"\nContent-Type: application/octet-stream\n\n".bytes())
       .chain(file_content.into_iter())
       .chain("\n--".bytes())
       .chain(boundary.bytes())
       .chain("--".bytes())
       .collect())
}
fn file_name(f: &str) -> &str {
    Path::new(f).file_name().unwrap().to_str().unwrap()
}
fn file_extension(f: &str) -> &str {
    Path::new(f).extension().unwrap().to_str().unwrap()
}
fn make_url(url: &str, components: Vec<&str>) -> String {
    let mut res = String::new();
    res.push_str(url);

    for component in components.iter() {
        res.push('/');
        res.push_str(component);
    }

    res
}

impl Pc3Client {
    fn new(url: &str) -> Pc3Client {
        Pc3Client {
            url: url.to_string(),
            jar: CookieJar::new(url.as_bytes())
        }
    }

    fn authenticate(&mut self, user: &str, pass: &str) -> Result<(), Pc3Error> {
        let mut headers = Headers::new();
        headers.set(header::ContentType("application/x-www-form-urlencoded".parse().unwrap()));

        let mut client = Client::new();
        let body = format!("username={}&password={}", user, pass);
        let res = try!(client
                       .post(&make_url(&self.url, vec!["authenticate"])[..])
                       .headers(headers)
                       .body(&body[..])
                       .send());

        if let Some(&header::SetCookie(ref cookies)) = res.headers.get() {
            for cookie in cookies {
                self.jar.add(cookie.clone());
            }

            Ok(())
        } else {
            Err(Pc3Error::Other("Did not receive authentication cookie"))
        }
    }
    fn compete(&self, problem_name: &str, mut src: &mut File, src_name: &str) -> Result<Result<i32, ()>, Pc3Error> {
        if let Some(session) = self.jar.find("session") {
            let boundary = rand::thread_rng().gen_ascii_chars().take(48).collect::<String>();

            let mut headers = Headers::new();
            headers.set_raw("Content-Type", vec![b"multipart/form-data; boundary=".to_vec(), boundary.as_bytes().to_vec()]);
            headers.set_raw("Cookie", vec![b"session=".to_vec(), session.value.bytes().collect()]);

            let mut client = Client::new();
            let body = unsafe {String::from_utf8_unchecked(try!(create_submit_body(&boundary, src, src_name)))};
            let mut res = try!(client
                               .post(&make_url(&self.url, vec!["compete", problem_name, file_extension(src_name)])[..])
                               .headers(headers)
                               .body(&body[..])
                               .send());

            let mut result = String::new();
            try!(res.read_to_string(&mut result));
            let (success, score) = try!(json::decode::<(bool, i32)>(&result));

            if success {
                Ok(Ok(score))
            } else {
                Ok(Err(()))
            }
        } else {
            // No cookie from logging in
            Err(Pc3Error::Other("Not authenticated"))
        }
    }
    fn scores(&self) -> Result<Vec<(String, i32)>, Pc3Error> {
        let mut client = Client::new();
        let mut res = try!(client
                           .get(&make_url(&self.url, vec!["scores"])[..])
                           .send());

        let mut result = String::new();
        try!(res.read_to_string(&mut result));
        Ok(try!(json::decode::<Vec<(String, i32)>>(&result)))
    }
    fn inform(&self, problem_name: &str) -> Result<String, Pc3Error> {
        let mut client = Client::new();
        let mut res = try!(client
                           .get(&make_url(&self.url, vec!["inform", problem_name])[..])
                           .send());

        let mut result = String::new();
        try!(res.read_to_string(&mut result));
        Ok(try!(json::decode::<String>(&result)))
    }
}

fn main() {
    let mut client = Pc3Client::new("http://127.0.0.1:5000/api");
    client.authenticate("team1", "password").unwrap();
    println!("{}", client.inform("problem1").unwrap());
    println!("{:?}", client.compete("problem1", &mut File::open("./resources/program.java").unwrap(), "./resources/program.java").unwrap());
    println!("{:?}", client.scores().unwrap());
}
