use std::collections::HashMap;
use doc::{DocSpan, DocElement, DelSpan, DelElement, AddSpan, AddElement, Atom, Op};
use doc::DocElement::*;
use doc::DelElement::*;
use doc::AddElement::*;
use std::borrow::ToOwned;
use std::cmp;

use apply_add;
use apply_delete;
use apply_operation;

struct DelSlice<'a> {
	head:Option<DelElement>,
	rest:&'a [DelElement],
}

impl<'a> DelSlice<'a> {
	fn new(span:&'a DelSpan) -> DelSlice {
		DelSlice {
			head: Some(span[0].clone()),
			rest: &span[1..],
		}
	}

	fn next(&mut self) -> DelElement  {
		let res = self.head.clone().unwrap();
		if self.rest.len() == 0 {
			self.head = None;
			self.rest = &[];
		} else {
			self.head = Some(self.rest[0].clone());
			self.rest = &self.rest[1..];
		}
		res
	}

	fn get_head(&self) -> DelElement {
		self.head.clone().unwrap()
	}

	fn is_done(&self) -> bool {
		self.head.is_none()
	}
}

struct AddSlice<'a> {
	head:Option<AddElement>,
	rest:&'a [AddElement],
}

impl<'a> AddSlice<'a> {
	fn new(span:&'a AddSpan) -> AddSlice {
		AddSlice {
			head: Some(span[0].clone()),
			rest: &span[1..],
		}
	}

	fn next(&mut self) -> AddElement  {
		let res = self.head.clone().unwrap();
		if self.rest.len() == 0 {
			self.head = None;
			self.rest = &[];
		} else {
			self.head = Some(self.rest[0].clone());
			self.rest = &self.rest[1..];
		}
		res
	}

	fn get_head(&self) -> AddElement {
		self.head.clone().unwrap()
	}

	fn is_done(&self) -> bool {
		self.head.is_none()
	}
}

fn del_place_chars(res:&mut DelSpan, count:usize) {
	if res.len() > 0 {
		let idx = res.len() - 1;
		if let &mut DelChars(ref mut prefix) = &mut res[idx] {
			*prefix += count;
			return;
		}
	}
	res.push(DelChars(count));
}

fn del_place_skip(res:&mut DelSpan, count:usize) {
	if res.len() > 0 {
		let idx = res.len() - 1;
		if let &mut DelSkip(ref mut prefix) = &mut res[idx] {
			*prefix += count;
			return;
		}
	}
	res.push(DelSkip(count));
}

fn del_place_any(res:&mut DelSpan, value:&DelElement) {
	match value {
		&DelChars(count) => {
			del_place_chars(res, count);
		},
		&DelSkip(count) => {
			del_place_skip(res, count);
		},
		_ => {
			res.push(value.clone());
		}
	}
}

fn add_place_chars(res:&mut AddSpan, value:String) {
	if res.len() > 0 {
		let idx = res.len() - 1;
		if let &mut AddChars(ref mut prefix) = &mut res[idx] {
			prefix.push_str(&value[..]);
			return;
		}
	}
	res.push(AddChars(value));
}

fn add_place_skip(res:&mut AddSpan, count:usize) {
	if res.len() > 0 {
		let idx = res.len() - 1;
		if let &mut AddSkip(ref mut prefix) = &mut res[idx] {
			*prefix += count;
			return;
		}
	}
	res.push(AddSkip(count));
}

fn add_place_any(res:&mut AddSpan, value:&AddElement) {
	match value {
		&AddChars(ref value) => {
			add_place_chars(res, value.clone());
		},
		&AddSkip(count) => {
			add_place_skip(res, count);
		},
		_ => {
			res.push(value.clone());
		}
	}
}

fn compose_del_del(avec:&DelSpan, bvec:&DelSpan) -> DelSpan {
	let mut res = Vec::with_capacity(avec.len() + bvec.len());

	let mut a = DelSlice::new(avec);
	let mut b = DelSlice::new(bvec);

	while !a.is_done() {
		match a.get_head() {
			DelSkip(acount) => {
				match b.head.clone() {
					Some(DelSkip(bcount)) => {
						res.push(DelSkip(cmp::min(acount, bcount)));
						if acount > bcount {
							a.head = Some(DelSkip(acount - bcount));
							b.next();
						} else if acount < bcount {
							b.head = Some(DelSkip(bcount - acount));
							a.next();
						} else {
							a.next();
							b.next();
						}
					},
					Some(DelWithGroup(ref span)) => {
						if acount > 1 {
							a.head = Some(DelSkip(acount - 1));
						} else {
							a.next();
						}
						res.push(b.next());
					},
					Some(DelChars(bcount)) => {
						del_place_any(&mut res, &DelChars(cmp::min(acount, bcount)));
						if acount > bcount {
							a.head = Some(DelSkip(acount - bcount));
							b.next();
						} else if acount < bcount {
							b.head = Some(DelChars(bcount - acount));
							a.next();
						} else {
							a.next();
							b.next();
						}
					},
					Some(DelGroup) => {
						if acount > 1 {
							a.head = Some(DelSkip(acount - 1));
						} else {
							a.next();
						}
						res.push(b.next());
					},
					None => {
						res.push(a.next());
					}
				}
			},
			DelWithGroup(ref span) => {
				match b.head.clone() {
					Some(DelSkip(bcount)) => {
						if bcount > 1 {
							b.head = Some(DelSkip(bcount - 1));
						} else {
							b.next();
						}
						res.push(a.next());
					},
					Some(DelWithGroup(ref bspan)) => {
						res.push(DelWithGroup(compose_del_del(span, bspan)));
						a.next();
						b.next();
					},
					Some(DelChars(bcount)) => {
						panic!("DelWithGroup vs DelChars is bad");
					},
					Some(DelGroup) => {
						a.next();
						res.push(b.next());
					},
					None => {
						res.push(a.next());
					}
				}
			},
			DelChars(count) => {
				del_place_any(&mut res, &DelChars(count));
				a.next();
			},
			DelGroup => {
				res.push(DelGroup);
				a.next();
			},
		}
	}

	if !b.is_done() {
		del_place_any(&mut res, &b.get_head());
		res.push_all(b.rest);
	}

	res
}

fn compose_add_add(avec:&AddSpan, bvec:&AddSpan) -> AddSpan {
	let mut res = Vec::with_capacity(avec.len() + bvec.len());

	let mut a = AddSlice::new(avec);
	let mut b = AddSlice::new(bvec);

	while !b.is_done() {
		match b.get_head() {
			AddChars(value) => {
				add_place_any(&mut res, &b.next());
			},
			AddSkip(bcount) => {
				match a.get_head() {
					AddChars(value) => {
						let len = value.chars().count();
						if bcount < len {
							add_place_any(&mut res, &AddChars(value[..bcount].to_owned()));
							a.head = Some(AddChars(value[bcount..].to_owned()));
							b.next();
						} else if bcount > len {
							add_place_any(&mut res, &a.next());
							b.head = Some(AddSkip(bcount - len));
						} else {
							add_place_any(&mut res, &a.get_head());
							a.next();
							b.next();
						}
					},
					AddSkip(acount) => {
						res.push(AddSkip(cmp::min(acount, bcount)));
						if acount > bcount {
							a.head = Some(AddSkip(acount - bcount));
							b.next();
						} else if acount < bcount {
							b.head = Some(AddSkip(bcount - acount));
							a.next();
						} else {
							a.next();
							b.next();
						}
					},
					AddWithGroup(span) => {
						res.push(a.next());
					},
					_ => {
						panic!("Unimplemented");
					}
				}
			},
			_ => {
				panic!("Unimplemented");
			},
		}
	}

	if !a.is_done() {
		add_place_any(&mut res, &a.get_head());
		res.push_all(a.rest);
	}

	res
}

fn compose_add_del(avec:&AddSpan, bvec:&DelSpan) -> Op {
	let mut delres = Vec::with_capacity(avec.len() + bvec.len());
	let mut addres = Vec::with_capacity(avec.len() + bvec.len());

	let mut a = AddSlice::new(avec);
	let mut b = DelSlice::new(bvec);

	while !b.is_done() {
		match b.get_head() {
			DelChars(bcount) => {
				match a.get_head() {
					AddChars(avalue) => {
						let alen = avalue.chars().count();
						if bcount < alen {
							a.head = Some(AddChars(avalue[bcount..].to_owned()));
							b.next();
						} else if bcount > alen {
							a.next();
							b.head = Some(DelChars(bcount - alen));
						} else {
							a.next();
							b.next();
						}
					},
					AddSkip(acount) => {
						if bcount < acount {
							a.head = Some(AddSkip(acount - bcount));
							del_place_any(&mut delres, &b.next());
						} else if bcount > acount {
							a.next();
							del_place_any(&mut delres, &DelChars(acount));
							b.head = Some(DelChars(bcount - acount));
						} else {
							a.next();
							del_place_any(&mut delres, &b.next());
						}
					},
					_ => {
						panic!("Unimplemented or Unexpected");
					},
				}
			},
			DelSkip(bcount) => {
				match a.get_head() {
					AddChars(avalue) => {
						let alen = avalue.chars().count();
						if bcount < alen {
							add_place_any(&mut addres, &AddChars(avalue[..bcount].to_owned()));
							a.head = Some(AddChars(avalue[bcount..].to_owned()));
							b.next();
						} else if bcount > alen {
							add_place_any(&mut addres, &a.next());
							b.head = Some(DelSkip(bcount - alen));
						} else {
							add_place_any(&mut addres, &a.get_head());
							a.next();
							b.next();
						}
					},
					AddSkip(acount) => {
						add_place_any(&mut addres, &AddSkip(cmp::min(acount, bcount)));
						del_place_any(&mut delres, &DelSkip(cmp::min(acount, bcount)));
						if acount > bcount {
							a.head = Some(AddSkip(acount - bcount));
							b.next();
						} else if acount < bcount {
							a.next();
							b.head = Some(DelSkip(bcount - acount));
						} else {
							a.next();
							b.next();
						}
					},
					AddWithGroup(span) => {
						addres.push(a.next());
					},
					_ => {
						panic!("Unimplemented");
					}
				}
			},
			_ => {
				panic!("Unimplemented");
			},
		}
	}

	if !a.is_done() {
		add_place_any(&mut addres, &a.get_head());
		addres.push_all(a.rest);
	}

	(delres, addres)
}



#[test]
fn test_compose_del_del() {
	assert_eq!(compose_del_del(&vec![
		DelSkip(6),
		DelChars(6),
	], &vec![
		DelChars(3),
	]), vec![
		DelChars(3),
		DelSkip(3),
		DelChars(6),
	]);

	assert_eq!(compose_del_del(&vec![
		DelSkip(6),
		DelChars(6),
	], &vec![
		DelChars(6),
	]), vec![
		DelChars(12),
	]);

	assert_eq!(compose_del_del(&vec![
		DelWithGroup(vec![
			DelChars(6),
		]),
	], &vec![
		DelGroup,
	]), vec![
		DelGroup,
	]);

	assert_eq!(compose_del_del(&vec![
		DelWithGroup(vec![
			DelChars(6),
		]),
	], &vec![
		DelWithGroup(vec![
			DelChars(6),
		]),
	]), vec![
		DelWithGroup(vec![
			DelChars(12),
		]),
	]);

	assert_eq!(compose_del_del(&vec![
		DelSkip(2), DelChars(6), DelSkip(1), DelChars(2), DelSkip(1)
	], &vec![
		DelSkip(1), DelChars(1), DelSkip(1)
	]), vec![
		DelSkip(1), DelChars(7), DelSkip(1), DelChars(2), DelSkip(1)
	]);
}

#[test]
fn test_compose_add_add() {
	assert_eq!(compose_add_add(&vec![
		AddChars("World!".to_owned()),
	], &vec![
		AddChars("Hello ".to_owned()),
	]), vec![
		AddChars("Hello World!".to_owned()),
	]);

	assert_eq!(compose_add_add(&vec![
		AddChars("edef".to_owned()),
	], &vec![
		AddChars("d".to_owned()),
		AddSkip(1),
		AddChars("a".to_owned()),
		AddSkip(1),
		AddChars("b".to_owned()),
		AddSkip(1),
		AddChars("e".to_owned()),
		AddSkip(1),
	]), vec![
		AddChars("deadbeef".to_owned()),
	]);

	assert_eq!(compose_add_add(&vec![
		AddSkip(10),
		AddChars("h".to_owned()),
	], &vec![
		AddSkip(11),
		AddChars("i".to_owned()),
	]), vec![
		AddSkip(10),
		AddChars("hi".to_owned()),
	]);

	assert_eq!(compose_add_add(&vec![
		AddSkip(5), AddChars("yEH".to_owned()), AddSkip(1), AddChars("GlG5".to_owned()), AddSkip(4), AddChars("nnG".to_owned()), AddSkip(1), AddChars("ra8c".to_owned()), AddSkip(1)
	], &vec![
		AddSkip(10), AddChars("Eh".to_owned()), AddSkip(16),
	]), vec![
		AddSkip(5), AddChars("yEH".to_owned()), AddSkip(1), AddChars("GEhlG5".to_owned()), AddSkip(4), AddChars("nnG".to_owned()), AddSkip(1), AddChars("ra8c".to_owned()), AddSkip(1)
	]);
}

use rand::{thread_rng, Rng};

fn random_add_span(input:&DocSpan) -> AddSpan {
	let mut rng = thread_rng();

	let mut res = vec![];
	for elem in input {
		match elem {
			&DocChars(ref value) => {
				let mut n = 0;
				let max = value.chars().count();
				while n < max {
					let slice = rng.gen_range(1, max - n + 1);
					res.push(AddSkip(slice));
					if slice < max - n || rng.gen_weighted_bool(2) {
						let len = rng.gen_range(1, 5);
						res.push(AddChars(rng.gen_ascii_chars().take(len).collect()));
					}
					n += slice;
				}
			},
			_ => {
				panic!("Unexpected");
			}
		}
	}
	res
}


fn random_del_span(input:&DocSpan) -> DelSpan {
	let mut rng = thread_rng();

	let mut res = vec![];
	for elem in input {
		match elem {
			&DocChars(ref value) => {
				let mut n = 0;
				let max = value.chars().count();
				while n < max {
					if max - n == 1 {
						res.push(DelSkip(1));
						n += 1;
					} else {
						let slice = rng.gen_range(2, max - n + 1);
						if slice == 2 {
							res.push(DelSkip(1));
							res.push(DelChars(1));
							n += 2;
						} else {
							let keep = rng.gen_range(1, slice - 1);
							res.push(DelSkip(keep));
							res.push(DelChars(slice - keep));
							n += slice;
						}
					}
				}
			},
			_ => {
				panic!("Unexpected");
			}
		}
	}
	res
}

#[test]
fn monkey_add_add() {
	for i in 0..1000 {
		let start = vec![
			DocChars("Hello world!".to_owned()),
		];

		println!("start {:?}", start);

		let a = random_add_span(&start);
		println!("a {:?}", a);

		let middle = apply_add(&start, &a);
		let b = random_add_span(&middle);
		let end = apply_add(&middle, &b);

		let composed = compose_add_add(&a, &b);
		let otherend = apply_add(&start, &composed);

		println!("middle {:?}", middle);
		println!("b {:?}", b);
		println!("end {:?}", end);

		println!("composed {:?}", composed);
		println!("otherend {:?}", otherend);

		assert_eq!(end, otherend);
	}
}

#[test]
fn monkey_del_del() {
	for i in 0..1000 {
		let start = vec![
			DocChars("Hello world!".to_owned()),
		];

		println!("start {:?}", start);

		let a = random_del_span(&start);
		println!("a {:?}", a);

		let middle = apply_delete(&start, &a);
		let b = random_del_span(&middle);
		let end = apply_delete(&middle, &b);

		let composed = compose_del_del(&a, &b);
		let otherend = apply_delete(&start, &composed);

		println!("middle {:?}", middle);
		println!("b {:?}", b);
		println!("end {:?}", end);

		println!("composed {:?}", composed);
		println!("otherend {:?}", otherend);

		assert_eq!(end, otherend);
	}
}

#[test]
fn test_compose_add_del() {
	assert_eq!(compose_add_del(&vec![
		AddSkip(4), AddChars("0O".to_owned()), AddSkip(5), AddChars("mnc".to_owned()), AddSkip(3), AddChars("gbL".to_owned()),
	], &vec![
		DelSkip(1), DelChars(1), DelSkip(3), DelChars(2), DelSkip(2), DelChars(9), DelSkip(1), DelChars(1),
	]), (vec![
		DelSkip(1), DelChars(1), DelSkip(2), DelChars(1), DelSkip(2), DelChars(5),
	], vec![
		AddSkip(3), AddChars("0".to_owned()), AddSkip(2), AddChars("b".to_owned()),
	]));
}

#[test]
fn monkey_add_del() {
	for i in 0..1000 {
		let start = vec![
			DocChars("Hello world!".to_owned()),
		];

		println!("start {:?}", start);

		let a = random_add_span(&start);
		println!("a {:?}", a);

		let middle = apply_add(&start, &a);
		let b = random_del_span(&middle);
		let end = apply_delete(&middle, &b);

		println!("middle {:?}", middle);
		println!("b {:?}", b);
		println!("end {:?}", end);

		let (dela, addb) = compose_add_del(&a, &b);
		println!("dela {:?}", dela);
		println!("addb {:?}", addb);

		let middle2 = apply_delete(&start, &dela);
		println!("middle2 {:?}", middle2);
		let otherend = apply_add(&middle2, &addb);
		println!("otherend {:?}", otherend);

		assert_eq!(end, otherend);
	}
}

fn compose(a:&Op, b:&Op) -> Op {
	let &(ref adel, ref ains) = a;
	let &(ref bdel, ref bins) = b;

	let (mdel, mins) = compose_add_del(ains, bdel);
	(compose_del_del(adel, &mdel), compose_add_add(&mins, bins))
}

fn random_op(input:&DocSpan) -> Op {
	let del = random_del_span(input);
	let middle = apply_delete(input, &del);
	let ins = random_add_span(&middle);
	(del, ins)
}


#[test]
fn monkey_compose() {
	for i in 0..1000 {
		let start = vec![
			DocChars("Hello world!".to_owned()),
		];

		println!("start {:?}", start);

		let a = random_op(&start);
		println!("a {:?}", a);

		let middle = apply_operation(&start, &a);
		println!("middle {:?}", middle);

		let b = random_op(&middle);
		println!("b {:?}", b);

		let end = apply_operation(&middle, &b);
		println!("end {:?}", end);

		let composed = compose(&a, &b);
		println!("composed {:?}", composed);

		let otherend = apply_operation(&start, &composed);
		println!("otherend {:?}", otherend);

		assert_eq!(end, otherend);
	}
}
