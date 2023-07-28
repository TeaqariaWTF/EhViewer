mod parser;

extern crate android_logger;
extern crate apply;
extern crate catch_panic;
extern crate jni_fn;
extern crate jnix;
extern crate jnix_macros;
extern crate log;
extern crate once_cell;
extern crate quick_xml;
extern crate regex;
extern crate tl;

use android_logger::Config;
use apply::Also;
use catch_panic::catch_panic;
use jni_fn::jni_fn;
use jnix::jni::objects::{JClass, JString};
use jnix::jni::sys::{jint, jintArray, jobjectArray, JavaVM, JNI_VERSION_1_6};
use jnix::jni::JNIEnv;
use jnix::JnixEnv;
use log::LevelFilter;
use quick_xml::escape::unescape;
use std::ffi::c_void;
use tl::{Bytes, Node, NodeHandle, Parser, VDom};

#[macro_export]
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

const EHGT_PREFIX: &str = "https://ehgt.org/";
const EX_PREFIX: &str = "https://s.exhentai.org/";

fn get_vdom_first_element_by_class_name<'a>(dom: &'a VDom, name: &str) -> Option<&'a Node<'a>> {
    let handle = dom.get_elements_by_class_name(name).next()?;
    handle.get(dom.parser())
}

fn get_first_element_by_class_name<'a>(
    node: &'a Node,
    parser: &'a Parser,
    id: &str,
) -> Option<&'a Node<'a>> {
    let handle = node.find_node(parser, &mut |n| match n.as_tag() {
        None => false,
        Some(tag) => tag.attributes().is_class_member(id),
    })?;
    handle.get(parser)
}

fn get_element_by_id<'b, S>(node: &'b Node, parser: &'b Parser, id: S) -> Option<&'b Node<'b>>
where
    S: Into<Bytes<'b>>,
{
    let bytes: Bytes = id.into();
    let handle = node.find_node(parser, &mut |n| match n.as_tag() {
        None => false,
        Some(tag) => tag.attributes().id().map_or(false, |x| x.eq(&bytes)),
    })?;
    handle.get(parser)
}

fn parse_jni_string<F, R>(env: &mut JnixEnv, str: &JString, mut f: F) -> Option<R>
where
    F: FnMut(&VDom, &Parser, &JnixEnv) -> Option<R>,
{
    let html = env.get_string(*str).ok()?;
    let dom = tl::parse(html.to_str().ok()?, tl::ParserOptions::default()).ok()?;
    let parser = dom.parser();
    f(&dom, parser, env)
}

fn get_node_handle_attr<'a>(
    node: &NodeHandle,
    parser: &'a Parser,
    attr: &'a str,
) -> Option<&'a str> {
    get_node_attr(node.get(parser)?, attr)
}

fn get_node_attr<'a>(node: &'a Node<'_>, attr: &'a str) -> Option<&'a str> {
    let str = node.as_tag()?.attributes().get(attr)??.try_as_utf8_str()?;
    Some(str)
}

// Should not use it at too upper node, since it will do DFS?
fn query_childs_first_match_attr<'a>(
    node: &'a Node<'_>,
    parser: &'a Parser,
    attr: &'a str,
) -> Option<&'a str> {
    let selector = format!("[{}]", attr);
    let mut iter = node.as_tag()?.query_selector(parser, &selector)?;
    get_node_handle_attr(&iter.next()?, parser, attr)
}

#[no_mangle]
#[catch_panic(default = "std::ptr::null_mut()")]
#[allow(non_snake_case)]
#[jni_fn("com.hippo.ehviewer.client.parser.FavoritesParserKt")]
pub fn parseFav(env: JNIEnv, _class: JClass, input: JString, str: jobjectArray) -> jintArray {
    let mut env = JnixEnv { env };
    let vec = parse_jni_string(&mut env, &input, |dom, parser, env| {
        let vec: Vec<i32> = dom
            .get_elements_by_class_name("fp")
            .enumerate()
            .filter_map(|(i, e)| {
                if i == 10 {
                    return None;
                }
                let top = e.get(parser)?.children()?;
                let children = top.top();
                let cat = children[5].get(parser)?.inner_text(parser);
                let name = unescape(&cat).ok()?;
                env.set_object_array_element(str, i as i32, env.new_string(name.trim()).ok()?)
                    .ok()?;
                children[1]
                    .get(parser)?
                    .inner_text(parser)
                    .parse::<i32>()
                    .ok()
            })
            .collect();
        if vec.len() == 10 {
            Some(vec)
        } else {
            None
        }
    })
    .unwrap();
    env.new_int_array(10)
        .unwrap()
        .also(|it| env.set_int_array_region(*it, 0, &vec).unwrap())
}

#[no_mangle]
pub extern "system" fn JNI_OnLoad(_: JavaVM, _: *mut c_void) -> jint {
    android_logger::init_once(Config::default().with_max_level(LevelFilter::Debug));
    JNI_VERSION_1_6
}
