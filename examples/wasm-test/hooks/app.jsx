import theme from '@theme'
import styler from 'relay-runtime/themed-styler'
import relayRuntime from '@clevertree/relay-runtime'
import hooksManifest from '@hooks'

/**
 * App hook — orchestrates theme loading, hook execution, and CSS injection.
 * Theme toggle via @clevertree/relay-runtime (getThemes); setCurrentTheme handled by event delegation.
 */
export default async function App() {
  let state = styler.get_default_state_json();

  if (theme && theme.themes) {
    const entries = styler.objectEntries(theme.themes);
    for (let i = 0; i < entries.length; i++) {
      const entry = entries[i];
      const name = entry[0];
      const themeData = entry[1];
      const themeJson = styler.buildThemeJson(name, themeData);
      state = styler.register_theme_json(state, themeJson);
    }
    state = styler.set_theme_json(state, 'default');
  }

  let sectionNames = [];
  let sectionVnodes = [];
  let allTags = [];
  let allClasses = [];
  let allTagClasses = [];

  if (hooksManifest && hooksManifest.hooks) {
    const hooks = hooksManifest.hooks;
    for (let i = 0; i < hooks.length; i = i + 1) {
      const hook = hooks[i];
      const mod = await import(hook.path);
      const vnode = mod.default();
      const vnodeStr = styler.stringifyJson(vnode);
      const usage = styler.parseJson(styler.collectUsage(vnodeStr));
      if (usage.tags) {
        for (let j = 0; j < usage.tags.length; j++) {
          allTags = styler.arrayConcat(allTags, [usage.tags[j]]);
        }
      }
      if (usage.classes) {
        for (let j = 0; j < usage.classes.length; j++) {
          allClasses = styler.arrayConcat(allClasses, [usage.classes[j]]);
        }
      }
      if (usage.tag_classes) {
        for (let j = 0; j < usage.tag_classes.length; j++) {
          allTagClasses = styler.arrayConcat(allTagClasses, [usage.tag_classes[j]]);
        }
      }
      sectionNames = styler.arrayConcat(sectionNames, [hook.name]);
      sectionVnodes = styler.arrayConcat(sectionVnodes, [vnode]);
    }
  }

  const stateWithUsage = styler.mergeUsageIntoState(state, allTags, allClasses, allTagClasses);
  const css = styler.render_css_for_web(stateWithUsage);
  styler.injectStyles(css);

  let themeNames = [];
  const themesJson = relayRuntime.getThemes();
  const themesArr = styler.parseJson(themesJson);
  if (themesArr && themesArr.length) {
    for (let i = 0; i < themesArr.length; i++) {
      const t = themesArr[i];
      const key = t.key;
      if (key) themeNames = styler.arrayConcat(themeNames, [key]);
    }
  }

  let children = [];
  if (themeNames.length > 0) {
    let toggleButtons = [];
    for (let i = 0; i < themeNames.length; i++) {
      const name = themeNames[i];
      toggleButtons = styler.arrayConcat(toggleButtons, [
        <button className="theme-toggle-btn" dataTheme={name}>
          {name}
        </button>
      ]);
    }
    children = styler.arrayConcat(children, [
      <div className="theme-toggle">
        {toggleButtons}
      </div>
    ]);
  }
  for (let i = 0; i < sectionNames.length; i++) {
    const name = sectionNames[i];
    const vnode = sectionVnodes[i];
    children = styler.arrayConcat(children, [
      <div className="test-section">
        <div className="test-header">{name}</div>
        <div className="test-output">{vnode}</div>
      </div>
    ]);
  }
  let result = children;
  if (children.length === 0) result = [<div>No content</div>];
  return result;
}
