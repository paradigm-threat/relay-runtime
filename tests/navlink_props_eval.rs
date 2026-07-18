//! NavLink-style local component must receive string JSX props.
use relay_runtime::host::{HostHandle, MockHost};
use relay_runtime::run_state::{clear_run_state, run_hook_persistent};
use relay_runtime::vdom::{value_to_vnode, PropValue, VNode};

fn null_host() -> HostHandle {
    MockHost::new()
}

fn find_links(v: &VNode, out: &mut Vec<(String, String)>) {
    match v {
        VNode::Element {
            tag,
            props,
            children,
            ..
        } => {
            if tag == "a" {
                let href = props
                    .get("href")
                    .and_then(|p| match p {
                        PropValue::String(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut label = String::new();
                for c in children {
                    match c {
                        VNode::Element {
                            tag,
                            children: kids,
                            ..
                        } if tag == "span" => {
                            for k in kids {
                                if let VNode::Text(t) = k {
                                    label.push_str(t);
                                }
                            }
                        }
                        VNode::Text(t) => label.push_str(t),
                        _ => {}
                    }
                }
                out.push((href, label));
            }
            for c in children {
                find_links(c, out);
            }
        }
        VNode::Text(_) | VNode::Empty => {}
    }
}

#[test]
fn local_navlink_receives_string_props() {
    clear_run_state();
    let src = r#"
function NavLink(props) {
  let href = '#'
  let label = ''
  try { if (props.href) href = props.href } catch (e) {}
  try { if (props.label) label = props.label } catch (e) {}
  return <a href={href} className="pt-nav-link"><span>{label}</span></a>
}
export default function App() {
  return (
    <nav>
      <NavLink href="/" label="Home" />
      <NavLink href="/search" label="Search" />
    </nav>
  )
}
"#;
    let result = run_hook_persistent(src, null_host()).expect("run");
    let vnode = value_to_vnode(&result).normalize();
    let mut links = Vec::new();
    find_links(&vnode, &mut links);
    assert_eq!(links.len(), 2, "links={links:?}");
    assert_eq!(links[0], ("/".into(), "Home".into()));
    assert_eq!(links[1], ("/search".into(), "Search".into()));
}

#[test]
fn local_navlink_with_icon_jsx_prop() {
    clear_run_state();
    let src = r#"
function IconHome() {
  return <svg><path d="M1" /></svg>
}
function NavLink(props) {
  let href = '#'
  let label = ''
  let icon = null
  try { if (props.href) href = props.href } catch (e) {}
  try { if (props.label) label = props.label } catch (e) {}
  try { if (props.icon) icon = props.icon } catch (e) {}
  return <a href={href} className="pt-nav-link">{icon}<span>{label}</span></a>
}
export default function App() {
  return <NavLink href="/" label="Home" icon={<IconHome />} />
}
"#;
    let result = run_hook_persistent(src, null_host()).expect("run");
    let vnode = value_to_vnode(&result).normalize();
    let mut links = Vec::new();
    find_links(&vnode, &mut links);
    assert_eq!(links.len(), 1, "links={links:?} vnode={vnode:?}");
    assert_eq!(links[0], ("/".into(), "Home".into()));
}

#[test]
fn propor_dynamic_key_reads_href() {
    clear_run_state();
    let src = r#"
function propOr(props, key, fallback) {
  try {
    const v = props[key]
    if (v === null) return fallback
    if (typeof v === 'undefined') return fallback
    if (typeof v === 'string' && v.length === 0) return fallback
    return v
  } catch (e) {
    return fallback
  }
}
function NavLink(props) {
  const href = propOr(props, 'href', '#')
  const label = propOr(props, 'label', '')
  return <a href={href} className="pt-nav-link"><span>{label}</span></a>
}
export default function App() {
  return <NavLink href="/" label="Home" />
}
"#;
    let result = run_hook_persistent(src, null_host()).expect("run");
    let vnode = value_to_vnode(&result).normalize();
    let mut links = Vec::new();
    find_links(&vnode, &mut links);
    assert_eq!(links.len(), 1, "links={links:?}");
    assert_eq!(links[0], ("/".into(), "Home".into()), "propOr must read static props");
}
