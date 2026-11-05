// Copyright 2026 Cloudflare, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::*;
use bytes::Bytes;
use http::header;
use http::header::InvalidHeaderName;

#[derive(Debug, Clone)]
pub struct CaseHeaderName(Bytes);

impl CaseHeaderName {
    pub fn new(name: String) -> Self {
        CaseHeaderName(name.into())
    }
}

impl CaseHeaderName {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn from_slice(buf: &[u8]) -> Self {
        CaseHeaderName(Bytes::copy_from_slice(buf))
    }
}

/// A trait that converts into case-sensitive header names.
pub trait IntoCaseHeaderName {
    fn into_case_header_name(self) -> CaseHeaderName;

    /// Returns the header name together with the case to write it in.
    ///
    /// The default parses the name from its case. Types that hold a parsed name already return
    /// it instead, which saves parsing, and for names that are not standard, an allocation.
    fn into_name_and_case(self) -> Result<(HeaderName, CaseHeaderName), InvalidHeaderName>
    where
        Self: Sized,
    {
        let case = self.into_case_header_name();
        let name = HeaderName::from_bytes(case.as_slice())?;
        Ok((name, case))
    }
}

/// A header name parsed ahead of time, with the case to write it in over HTTP/1.x.
///
/// Adding a header under it neither parses the name nor allocates for it, and cloning one made
/// with [`CasedHeaderName::from_static`] copies two pointers, so it suits names that are added to
/// every request or response.
///
/// ```
/// use pingora_http::{CasedHeaderName, RequestHeader};
///
/// static X_REQUEST_ID: CasedHeaderName = CasedHeaderName::from_static("x-request-id", "X-Request-Id");
///
/// let mut request = RequestHeader::build("GET", b"/", None).unwrap();
/// request.insert_header(&X_REQUEST_ID, "abc").unwrap();
/// assert_eq!(request.headers["x-request-id"], "abc");
/// ```
#[derive(Debug, Clone)]
pub struct CasedHeaderName {
    name: HeaderName,
    case: CaseHeaderName,
}

impl CasedHeaderName {
    /// Makes a header name from `name`, in lowercase as [`HeaderName::from_static`] wants it,
    /// written as `case` over HTTP/1.x.
    ///
    /// # Panics
    ///
    /// If `name` is not a valid header name in lowercase, or `case` is not `name` in some case.
    /// In a constant or a static, that fails the build instead.
    pub const fn from_static(name: &'static str, case: &'static str) -> Self {
        let (lower, cased) = (name.as_bytes(), case.as_bytes());
        assert!(
            lower.len() == cased.len(),
            "the case of a header name differs in length"
        );
        let mut i = 0;
        while i < lower.len() {
            assert!(
                lower[i] == cased[i].to_ascii_lowercase(),
                "the case of a header name is another name"
            );
            i += 1;
        }
        CasedHeaderName {
            name: HeaderName::from_static(name),
            case: CaseHeaderName(Bytes::from_static(case.as_bytes())),
        }
    }

    /// The header name.
    pub fn name(&self) -> &HeaderName {
        &self.name
    }
}

impl IntoCaseHeaderName for &CasedHeaderName {
    fn into_case_header_name(self) -> CaseHeaderName {
        self.case.clone()
    }

    fn into_name_and_case(self) -> Result<(HeaderName, CaseHeaderName), InvalidHeaderName> {
        Ok((self.name.clone(), self.case.clone()))
    }
}

impl IntoCaseHeaderName for CaseHeaderName {
    fn into_case_header_name(self) -> CaseHeaderName {
        self
    }
}

impl IntoCaseHeaderName for String {
    fn into_case_header_name(self) -> CaseHeaderName {
        CaseHeaderName(self.into())
    }
}

impl IntoCaseHeaderName for &'static str {
    fn into_case_header_name(self) -> CaseHeaderName {
        CaseHeaderName(self.into())
    }
}

impl IntoCaseHeaderName for HeaderName {
    fn into_case_header_name(self) -> CaseHeaderName {
        CaseHeaderName(titled_header_name(&self))
    }

    fn into_name_and_case(self) -> Result<(HeaderName, CaseHeaderName), InvalidHeaderName> {
        let case = CaseHeaderName(titled_header_name(&self));
        Ok((self, case))
    }
}

impl IntoCaseHeaderName for &HeaderName {
    fn into_case_header_name(self) -> CaseHeaderName {
        CaseHeaderName(titled_header_name(self))
    }

    fn into_name_and_case(self) -> Result<(HeaderName, CaseHeaderName), InvalidHeaderName> {
        Ok((self.clone(), CaseHeaderName(titled_header_name(self))))
    }
}

impl IntoCaseHeaderName for Bytes {
    fn into_case_header_name(self) -> CaseHeaderName {
        CaseHeaderName(self)
    }
}

fn titled_header_name(header_name: &HeaderName) -> Bytes {
    titled_header_name_str(header_name).map_or_else(
        || Bytes::copy_from_slice(header_name.as_str().as_bytes()),
        |s| Bytes::from_static(s.as_bytes()),
    )
}

pub(crate) fn titled_header_name_str(header_name: &HeaderName) -> Option<&'static str> {
    Some(match *header_name {
        header::ACCEPT_RANGES => "Accept-Ranges",
        header::AGE => "Age",
        header::CACHE_CONTROL => "Cache-Control",
        header::CONNECTION => "Connection",
        header::CONTENT_TYPE => "Content-Type",
        header::CONTENT_ENCODING => "Content-Encoding",
        header::CONTENT_LENGTH => "Content-Length",
        header::DATE => "Date",
        header::TRANSFER_ENCODING => "Transfer-Encoding",
        header::HOST => "Host",
        header::SERVER => "Server",
        header::SET_COOKIE => "Set-Cookie",
        // TODO: add more const header here to map to their titled case
        // TODO: automatically upper case the first letter?
        _ => {
            return None;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_header_name() {
        assert_eq!("FoO".into_case_header_name().as_slice(), b"FoO");
        assert_eq!("FoO".to_string().into_case_header_name().as_slice(), b"FoO");
        assert_eq!(header::SERVER.into_case_header_name().as_slice(), b"Server");
    }

    #[test]
    fn test_name_and_case() {
        let (name, case) = "X-Foo".into_name_and_case().unwrap();
        assert_eq!((name.as_str(), case.as_slice()), ("x-foo", &b"X-Foo"[..]));
        assert!("X Foo".into_name_and_case().is_err());

        let (name, case) = header::SERVER.into_name_and_case().unwrap();
        assert_eq!((name, case.as_slice()), (header::SERVER, &b"Server"[..]));
        let custom = HeaderName::from_static("x-foo");
        let (name, case) = (&custom).into_name_and_case().unwrap();
        assert_eq!((name, case.as_slice()), (custom, &b"x-foo"[..]));

        static X_FOO: CasedHeaderName = CasedHeaderName::from_static("x-foo", "X-FoO");
        let (name, case) = (&X_FOO).into_name_and_case().unwrap();
        assert_eq!((name.as_str(), case.as_slice()), ("x-foo", &b"X-FoO"[..]));
        assert_eq!(X_FOO.name(), "x-foo");
    }

    #[test]
    #[should_panic(expected = "another name")]
    fn test_cased_header_name_of_another_name() {
        CasedHeaderName::from_static("x-foo", "X-Bar");
    }
}
