#[macro_use] extern crate oatie;
extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;

use std::mem;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use oatie::doc::*;
use oatie::writer::*;

fn default_doc() -> Doc {
    Doc(doc_span![
        DocGroup({"tag": "h1"}, [
            DocChars("Hello! "),
            DocGroup({"tag": "span", "class": "bold"}, [DocChars("what's")]),
            DocChars(" up?"),
        ]),
        DocGroup({"tag": "ul"}, [
            DocGroup({"tag": "li"}, [
                DocGroup({"tag": "p"}, [
                    DocChars("Three adjectives strong."),
                ]),
                DocGroup({"tag": "p"}, [
                    DocChars("World!"),
                ]),
            ]),
        ])
    ])
}

pub fn fact(n: u32) -> u64 {
    let mut n = n as u64;
    let mut result = 1;
    while n > 0 {
        result = result * n;
        n = n - 1;
    }
    result
}

fn rename_group_inner(input: &mut CurStepper, doc: &mut DocStepper, del: &mut DelWriter, add: &mut AddWriter) {
    while !input.is_done() && input.head.is_some() {
        match input.get_head() {
            CurSkip(value) => {
                doc.skip(value);
                input.next();
            }
            CurWithGroup(..) => {
                input.enter();
                doc.enter();
                rename_group_inner(input, doc, del, add);
                input.exit();
                doc.exit();
            }
            CurGroup => {
                // TODO
                input.next();
            }
        }
    }
}

fn rename_group(input: &CurSpan) {
    let doc = default_doc();

    let mut cur_stepper = CurStepper::new(input);
    let mut doc_stepper = DocStepper::new(&doc.0);
    let mut del_writer = DelWriter::new();
    let mut add_writer = AddWriter::new();
    rename_group_inner(&mut cur_stepper, &mut doc_stepper, &mut del_writer, &mut add_writer);
    println!("oh cool");
}






#[derive(Serialize, Deserialize, Debug)]
pub enum NativeRequest {
    Factorial(u32),
    RenameGroup(CurSpan),
    Invalid,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum NativeResponse {
    Factorial(u64),
    RenameGroup,
    Error(String),
}

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut c_void {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    return ptr as *mut c_void;
}

#[no_mangle]
pub extern "C" fn dealloc_str(ptr: *mut c_char) {
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

pub fn command_safe(req: NativeRequest) -> NativeResponse {
    match req {
        NativeRequest::RenameGroup(cur) => {
            rename_group(&cur);
            NativeResponse::RenameGroup
        }
        NativeRequest::Factorial(factor) => {
            NativeResponse::Factorial(fact(factor))
        }
        _ => {
            NativeResponse::Error("Invalid request".to_string())
        }
    }
}

#[no_mangle]
pub fn command(input_ptr: *mut c_char) -> *mut c_char {
    let input = unsafe {
        CString::from_raw(input_ptr)
    };
    let req_parse: Result<NativeRequest, _> = serde_json::from_slice(&input.into_bytes());

    let res = match req_parse {
        Ok(req) => command_safe(req),
        Err(err) => NativeResponse::Error(format!("{:?}", err)),
        // _ => command_safe(NativeRequest::Invalid),
    };

    let json = serde_json::to_string(&res).unwrap();
    let s = CString::new(json).unwrap();
    s.into_raw()
}