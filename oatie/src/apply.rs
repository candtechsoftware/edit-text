//! Methods to apply an operation to a document.

use super::doc::*;
use std::collections::HashMap;

fn apply_add_inner(spanvec: &DocSpan, delvec: &AddSpan) -> (DocSpan, DocSpan) {
    let mut span = &spanvec[..];
    let mut del = &delvec[..];

    let mut first = None;
    if !span.is_empty() {
        first = Some(span[0].clone());
        span = &span[1..]
    }

    let mut res: DocSpan = Vec::with_capacity(span.len());

    if del.is_empty() {
        return (vec![], spanvec.clone().to_vec());
    }

    let mut d = del[0].clone();
    del = &del[1..];

    let mut exhausted = first.is_none();

    trace!("ABOUT TO APPLY ADD {:?} {:?}", first, span);

    loop {
        // Flags for whether we have partially or fully consumed an atom.
        let mut nextdel = true;
        let mut nextfirst = true;

        if exhausted {
            match d {
                AddSkip(..) | AddWithGroup(..) => {
                    panic!("exhausted document on {:?}", d);
                }
                _ => {}
            }
        }

        trace!("next {:?} {:?} {:?}", d, first, exhausted);

        match d.clone() {
            AddStyles(count, styles) => match first.clone().unwrap() {
                DocChars(mut value) => {
                    if value.char_len() < count {
                        d = AddStyles(count - value.char_len(), styles.clone());
                        value.extend_styles(&styles);
                        res.place(&DocChars(value));
                        nextdel = false;
                    } else if value.char_len() > count {
                        let (mut left, right) = value.split_at(count);
                        left.extend_styles(&styles);
                        res.place(&DocChars(left));
                        first = Some(DocChars(right));
                        nextfirst = false;
                    } else {
                        value.extend_styles(&styles);
                        res.place(&DocChars(value));
                    }
                }
                DocGroup(..) => {
                    panic!("Invalid AddStyles");
                }
            },
            AddSkip(count) => match first.clone().unwrap() {
                DocChars(value) => {
                    if value.char_len() < count {
                        d = AddSkip(count - value.char_len());
                        res.place(&DocChars(value));
                        nextdel = false;
                    } else if value.char_len() > count {
                        let (left, right) = value.split_at(count);
                        res.place(&DocChars(left));
                        first = Some(DocChars(right));
                        nextfirst = false;
                    } else {
                        res.place(&DocChars(value));
                    }
                }
                DocGroup(..) => {
                    res.push(first.clone().unwrap());
                    if count > 1 {
                        d = AddSkip(count - 1);
                        nextdel = false;
                    }
                }
            },
            AddWithGroup(ref delspan) => match first.clone().unwrap() {
                DocGroup(ref attrs, ref span) => {
                    res.push(DocGroup(attrs.clone(), apply_add(span, delspan)));
                }
                _ => {
                    panic!("Invalid AddWithGroup");
                }
            },
            AddChars(value) => {
                res.place(&DocChars(value));
                nextfirst = false;
            }
            AddGroup(attrs, innerspan) => {
                let mut subdoc = vec![];
                if !exhausted {
                    subdoc.push(first.clone().unwrap());
                    subdoc.extend_from_slice(span);
                }
                trace!("CALLING INNER {:?} {:?}", subdoc, innerspan);

                let (inner, rest) = apply_add_inner(&subdoc, &innerspan);
                res.place(&DocGroup(attrs, inner));

                trace!("REST OF INNER {:?} {:?}", rest, del);

                let (inner, rest) = apply_add_inner(&rest, &del.to_vec());
                res.place_all(&inner);
                return (res, rest);
            }
        }

        if nextdel {
            if del.is_empty() {
                let mut remaining = vec![];
                trace!("nextfirst {:?} {:?} {:?}", nextfirst, first, exhausted);
                if !nextfirst && first.is_some() && !exhausted {
                    remaining.push(first.clone().unwrap());
                    // place_any(&mut res, &first.clone().unwrap());
                }
                remaining.extend_from_slice(span);
                return (res, remaining);
            }

            d = del[0].clone();
            del = &del[1..];
        }

        if nextfirst {
            if span.is_empty() {
                exhausted = true;
            } else {
                first = Some(span[0].clone());
                span = &span[1..];
            }
        }
    }
}

pub fn apply_add(spanvec: &DocSpan, delvec: &AddSpan) -> DocSpan {
    let (mut res, remaining) = apply_add_inner(spanvec, delvec);

    // TODO never accept unbalanced components?
    if !remaining.is_empty() {
        res.place_all(&remaining);
        // panic!("Unbalanced apply_add");
    }
    res
}

pub fn apply_delete(spanvec: &DocSpan, delvec: &DelSpan) -> DocSpan {
    let mut span = &spanvec[..];
    let mut del = &delvec[..];

    let mut res: DocSpan = Vec::with_capacity(span.len());

    if del.is_empty() {
        return span.to_vec();
    }

    let mut first = span[0].clone();
    span = &span[1..];

    let mut d = del[0].clone();
    del = &del[1..];

    loop {
        let mut nextdel = true;
        let mut nextfirst = true;

        match d.clone() {
            DelSkip(count) => match first.clone() {
                DocChars(value) => {
                    if value.char_len() < count {
                        d = DelSkip(count - value.char_len());
                        res.place(&DocChars(value));
                        nextdel = false;
                    } else if value.char_len() > count {
                        let (left, right) = value.split_at(count);
                        res.place(&DocChars(left));
                        first = DocChars(right);
                        nextfirst = false;
                    } else {
                        res.place(&DocChars(value));
                        nextdel = true;
                    }
                }
                DocGroup(..) => {
                    res.push(first.clone());
                    if count > 1 {
                        d = DelSkip(count - 1);
                        nextdel = false;
                    }
                }
            },
            DelWithGroup(ref delspan) => match first.clone() {
                DocGroup(ref attrs, ref span) => {
                    res.push(DocGroup(attrs.clone(), apply_delete(span, delspan)));
                }
                _ => {
                    panic!("Invalid DelWithGroup");
                }
            },
            DelGroup(ref delspan) => match first.clone() {
                DocGroup(ref attrs, ref span) => {
                    res.place_all(&apply_delete(span, delspan)[..]);
                }
                _ => {
                    panic!("Invalid DelGroup");
                }
            },
            DelChars(count) => match first.clone() {
                DocChars(ref value) => {
                    if value.char_len() > count {
                        let (_, right) = value.split_at(count);
                        first = DocChars(right);
                        nextfirst = false;
                    } else if value.char_len() < count {
                        panic!("attempted deletion of too much");
                    }
                }
                _ => {
                    panic!("Invalid DelChars");
                }
            }, // DelObject => {
               //     unimplemented!();
               // }
               // DelMany(count) => {
               //     match first.clone() {
               //         DocChars(ref value) => {
               //             let len = value.chars().count();
               //             if len > count {
               //                 first = DocChars(value.chars().skip(count).collect());
               //                 nextfirst = false;
               //             } else if len < count {
               //                 d = DelMany(count - len);
               //                 nextdel = false;
               //             }
               //         }
               //         DocGroup(..) => {
               //             if count > 1 {
               //                 d = DelMany(count - 1);
               //                 nextdel = false;
               //             } else {
               //                 nextdel = true;
               //             }
               //         }
               //     }
               // }
               // DelGroupAll => {
               //     match first.clone() {
               //         DocGroup(..) => {}
               //         _ => {
               //             panic!("Invalid DelGroupAll");
               //         }
               //     }
               // }
        }

        if nextdel {
            if del.is_empty() {
                if !nextfirst {
                    res.place(&first)
                }
                if !span.is_empty() {
                    res.place(&span[0]);
                    res.extend_from_slice(&span[1..]);
                }
                break;
            }

            d = del[0].clone();
            del = &del[1..];
        }

        if nextfirst {
            if span.is_empty() {
                panic!(
                    "exhausted document in apply_delete\n -->{:?}\n -->{:?}",
                    first, span
                );
            }

            first = span[0].clone();
            span = &span[1..];
        }
    }

    res
}

pub fn apply_operation(spanvec: &DocSpan, op: &Op) -> DocSpan {
    let &(ref delvec, ref addvec) = op;
    // println!("------> @1 {:?}", spanvec);
    // println!("------> @2 {:?}", delvec);
    let postdel = apply_delete(spanvec, delvec);
    // println!("------> @3 {:?}", postdel);
    // println!("------> @4 {:?}", addvec);
    apply_add(&postdel, addvec)
}

fn normalize_add_element(elem: AddElement) -> AddElement {
    match elem {
        AddGroup(attrs, span) => {
            let span = normalize_add_span(span, false);
            AddGroup(attrs, span)
        }
        AddWithGroup(span) => {
            let span = normalize_add_span(span, true);

            // Shortcut if the inner span is nothing but skips
            if span.is_empty() {
                AddSkip(1)
            } else {
                AddWithGroup(span)
            }
        }
        _ => elem,
    }
}

fn normalize_add_span(add: AddSpan, trim_last: bool) -> AddSpan {
    let mut ret: AddSpan = vec![];
    for elem in add.into_iter() {
        ret.place(&normalize_add_element(elem));
    }
    if trim_last {
        if let Some(&AddSkip(..)) = ret.last() {
            ret.pop();
        }
    }
    ret
}

fn normalize_del_element(elem: DelElement) -> DelElement {
    match elem {
        DelGroup(span) => {
            let span = normalize_del_span(span, false);
            DelGroup(span)
        }
        DelWithGroup(span) => {
            let span = normalize_del_span(span, true);

            // Shortcut if the inner span is nothing but skips
            if span.is_empty() {
                DelSkip(1)
            } else {
                DelWithGroup(span)
            }
        }
        _ => elem,
    }
}

fn normalize_del_span(del: DelSpan, trim_last: bool) -> DelSpan {
    let mut ret: DelSpan = vec![];
    for elem in del.into_iter() {
        ret.place(&normalize_del_element(elem));
    }
    if trim_last {
        if let Some(&DelSkip(..)) = ret.last() {
            ret.pop();
        }
    }
    ret
}

pub fn normalize(op: Op) -> Op {
    // TODO all
    (
        normalize_del_span(op.0, true),
        normalize_add_span(op.1, true),
    )
}
