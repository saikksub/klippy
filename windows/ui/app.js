'use strict';

// Renders one of Klippy's windows from the state the shell sends with
// klippy.render(), and sends the user's actions back over window.ipc.
// Copied content is only ever put in text nodes or a sandboxed srcdoc frame.

const windowKind = document.body.dataset.window;
const root = document.getElementById('root');

const post = (message) => window.ipc.postMessage(JSON.stringify(message));
/** Sends an event to the core, in serde's JSON form (e.g. {"CopyItem": 3}). */
const send = (event) => post({ type: 'event', event });
const openWindow = (window, tab) => post(tab ? { type: 'open', window, tab } : { type: 'open', window });
const closeWindow = () => post({ type: 'close' });

/** The latest state from the shell: { view, shell, selected, selected_unchanged }. */
let state = null;
/** The copy shown in History; updates that leave it out keep the last one. */
let selected = null;
/** Page-only state. */
const ui = { tab: document.body.dataset.tab, customRetention: false };

window.klippy = {
  render(next) {
    state = next;
    if (!next.selected_unchanged) selected = next.selected;
    render();
  },
  showTab(tab) {
    ui.tab = tab;
    render();
  },
};

// MARK: Helpers

const ICONS = {
  copy: '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V6a2 2 0 0 1 2-2h9"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  download: '<path d="M12 4v11m0 0l-4.5-4.5M12 15l4.5-4.5M5 19h14"/>',
  chevron: '<path d="M9.5 6l6 6-6 6"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
  shield: '<path d="M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6z"/><path d="M12 8v4.5M12 16h.01"/>',
  shieldCheck: '<path d="M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6z"/><path d="M8.5 12l2.5 2.5 4.5-5"/>',
  trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6"/>',
  clipboard: '<rect x="6" y="4" width="12" height="17" rx="2"/><path d="M9 4h6v3H9z"/>',
  share: '<path d="M12 15V4m0 0L7.5 8.5M12 4l4.5 4.5M5 13v6h14v-6"/>',
  pdf: '<path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5M9.5 13h5M9.5 17h5"/>',
  code: '<path d="M8.5 8L4.5 12l4 4M15.5 8l4 4-4 4"/>',
  text: '<path d="M5 6h14M5 10h14M5 14h10M5 18h7"/>',
  x: '<circle cx="12" cy="12" r="9"/><path d="M9 9l6 6M15 9l-6 6"/>',
};

function icon(name) {
  const template = document.createElement('template');
  template.innerHTML = `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`;
  return template.content.firstChild;
}

function appIcon() {
  return document.getElementById('app-icon').content.querySelector('svg').cloneNode(true);
}

/** document.createElement with properties (or attributes) and children. */
function h(tag, props = {}, ...children) {
  const element = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    // `false` still sets properties like `checked`, but skips attributes.
    if (value == null || (value === false && !(key in element))) continue;
    if (key.startsWith('on')) element.addEventListener(key.slice(2), value);
    else if (key === 'class') element.className = value;
    else if (key in element) element[key] = value;
    else element.setAttribute(key, value === true ? '' : value);
  }
  for (const child of children.flat()) {
    if (child == null || child === false) continue;
    element.append(child instanceof Node ? child : String(child));
  }
  return element;
}

/** Runs `action` without letting the click reach the row or close menus. */
const only = (action) => (event) => {
  event.stopPropagation();
  action();
};

const FORMATS = [
  ['Pdf', 'PDF', 'pdf'],
  ['Html', 'HTML', 'code'],
  ['Markdown', 'Markdown', 'text'],
];
const formatLabel = (format) => FORMATS.find(([value]) => value === format)[1];

const plural = (count, one, many) => (count === 1 ? one : many);

/** "Ctrl+C × 3": the shortcut times the configured count. */
function triggerBadge(view) {
  return h('span', { class: 'trigger', title: view.copy_trigger_hint },
    h('span', { class: 'kbd' }, 'Ctrl+C'), `× ${view.copy_trigger}`);
}

/** A rounded card for one history item. Hovering shows Copy and, when given, Export and Open. */
function row(item, { isSelected = false, onTap, onCopy, onExport, exportTitle, onOpen }) {
  const actions = h('div', { class: 'actions' });
  const copyButton = h('button', {
    class: 'icon-button',
    title: 'Copy',
    onclick: only(() => {
      onCopy();
      copyButton.replaceChildren(icon('check'));
      actions.classList.add('copied');
      setTimeout(() => {
        copyButton.replaceChildren(icon('copy'));
        actions.classList.remove('copied');
      }, 1000);
    }),
  }, icon('copy'));
  actions.append(copyButton);
  if (onExport) actions.append(h('button', { class: 'icon-button', title: exportTitle, onclick: only(onExport) }, icon('download')));
  if (onOpen) actions.append(h('button', { class: 'icon-button', title: 'Open in History', onclick: only(onOpen) }, icon('chevron')));
  return h('div', { class: isSelected ? 'row selected' : 'row', onclick: onTap },
    h('span', { class: 'preview' }, item.preview), actions);
}

function emptyState(iconName, title, text) {
  return h('div', { class: 'empty' }, iconName && icon(iconName), title && h('h2', {}, title), text && h('p', {}, text));
}

/** A drop-down of buttons under `anchorButton`. */
function menu(anchorButton, items) {
  const list = h('div', { class: 'menu hidden' },
    items.map((item) => (item === '-'
      ? h('hr')
      : h('button', { onclick: item.action }, item.label, item.shortcut && h('span', { class: 'shortcut' }, item.shortcut)))));
  anchorButton.addEventListener('click', only(() => {
    const wasHidden = list.classList.contains('hidden');
    closeMenus();
    list.classList.toggle('hidden', !wasHidden);
  }));
  return list;
}

function closeMenus() {
  document.querySelectorAll('.menu').forEach((list) => list.classList.add('hidden'));
}

/** Keeps a search field as the user typed it while it has focus. */
function syncInput(input, value) {
  if (document.activeElement !== input && input.value !== value) input.value = value;
}

// MARK: Main window

const main = { page: null, home: null };

function renderMain() {
  const view = state.view;
  if (view.page !== main.page) {
    main.page = view.page;
    const pages = { Splash: splashPage, Setup: setupPage, Home: homePage };
    root.replaceChildren(pages[view.page]());
  }
  if (view.page === 'Home') updateHome(view);
}

function splashPage() {
  return h('div', { class: 'center-page' }, h('div', { class: 'app-icon' }, appIcon()), h('h1', {}, 'Klippy'));
}

function setupPage() {
  return h('div', { class: 'center-page' }, h('h1', {}, 'Setup'), h('p', {}, "Klippy can't read copied text yet."));
}

function homePage() {
  const input = h('input', {
    type: 'search',
    placeholder: 'Search',
    spellcheck: false,
    oninput: (event) => send({ Search: event.target.value }),
  });
  const gear = h('button', { class: 'icon-button', title: 'Settings and Quit' }, icon('gear'));
  const gearMenu = menu(gear, [
    { label: 'Settings…', shortcut: 'Ctrl+,', action: () => openWindow('settings', 'general') },
    '-',
    { label: 'Quit Klippy', shortcut: 'Ctrl+Q', action: () => post({ type: 'quit' }) },
  ]);
  main.home = {
    input,
    info: h('div', { class: 'search-info hidden' }),
    list: h('div', { class: 'list' }),
    trigger: h('span'),
    failed: h('button', {
      class: 'failed-badge hidden',
      onclick: () => {
        send({ SetHistoryTab: 'Failed' });
        openWindow('history');
      },
    }),
    listKey: null,
  };
  const { info, list, trigger, failed } = main.home;
  return h('div', { class: 'main' },
    h('div', { class: 'search' }, input),
    info,
    list,
    h('div', { class: 'bottom-bar' }, trigger, h('span', { class: 'spacer' }), failed, gear, gearMenu));
}

function updateHome(view) {
  const home = main.home;
  syncInput(home.input, view.search);

  // The search as the core matches it (surrounding spaces ignored).
  const term = view.search.trim();
  const count = view.items.length;
  home.info.classList.toggle('hidden', !term);
  home.info.replaceChildren(
    h('div', { class: 'term' }, `You are searching for "${term}"`),
    h('div', { class: 'secondary' }, `Found ${count} ${plural(count, 'result', 'results')}`));

  home.trigger.replaceChildren(triggerBadge(view));

  const failed = view.failed_items.length;
  home.failed.classList.toggle('hidden', failed === 0);
  home.failed.replaceChildren(icon('shield'), String(failed));
  home.failed.title = failed === 1 ? '1 copy failed to save' : `${failed} copies failed to save`;

  const key = JSON.stringify([view.items, term, view.copy_trigger, view.quick_export_format]);
  if (key === home.listKey) return;
  home.listKey = key;
  if (count === 0) {
    // While searching, the info box above already says nothing was found.
    home.list.replaceChildren(term ? '' : h('div', { class: 'empty' }, triggerBadge(view), h('p', {}, 'to save a copy')));
    return;
  }
  home.list.replaceChildren(
    ...view.items.map((item) => row(item, {
      onTap: () => send({ CopyItem: item.id }),
      onCopy: () => send({ CopyItem: item.id }),
      onExport: () => send({ QuickExport: item.id }),
      exportTitle: `Export as ${formatLabel(view.quick_export_format)} to Downloads`,
      onOpen: () => {
        send({ SetHistoryTab: 'Saved' });
        send({ SelectItem: item.id });
        openWindow('history');
      },
    })),
    h('div', { class: 'end' }, 'End of the list'));
}

// MARK: History window

const hist = { mounted: false };

function mountHistory() {
  hist.search = h('input', {
    type: 'search',
    placeholder: 'Search saved copies',
    spellcheck: false,
    oninput: (event) => send({ HistorySearch: event.target.value }),
  });
  hist.list = h('div', { class: 'list' });
  hist.savedTab = h('button', { class: 'tab', onclick: () => send({ SetHistoryTab: 'Saved' }) });
  hist.failedTab = h('button', { class: 'tab', onclick: () => send({ SetHistoryTab: 'Failed' }) });
  hist.detail = h('div', { class: 'detail' });
  root.replaceChildren(h('div', { class: 'history' },
    h('div', { class: 'sidebar' },
      h('div', { class: 'search' }, hist.search),
      hist.list,
      h('div', { class: 'tabs' }, hist.savedTab, hist.failedTab)),
    hist.detail));
  hist.mounted = true;
}

function renderHistory() {
  if (!hist.mounted) mountHistory();
  const view = state.view;
  const tab = view.history_tab;
  const saved = tab === 'Saved';

  // Opening a tab with nothing selected shows its first item.
  if (saved && !selected && view.history_items.length) send({ SelectItem: view.history_items[0].id });
  if (!saved && !view.selected_failed && view.failed_items.length) send({ SelectFailed: view.failed_items[0].id });

  syncInput(hist.search, view.history_search);
  hist.search.parentElement.classList.toggle('hidden', !saved);

  hist.savedTab.className = saved ? 'tab active' : 'tab';
  hist.savedTab.replaceChildren(icon('clipboard'), 'Saved');
  hist.failedTab.className = saved ? 'tab' : 'tab active';
  const failedCount = view.failed_items.length;
  hist.failedTab.replaceChildren(icon('shield'), 'Failed', failedCount > 0 && h('span', { class: 'count' }, failedCount));

  const items = saved ? view.history_items : view.failed_items;
  const selectedId = saved ? selected?.id : view.selected_failed?.id;
  const listKey = JSON.stringify([tab, items, selectedId]);
  if (listKey !== hist.listKey) {
    hist.listKey = listKey;
    if (items.length === 0) {
      hist.list.replaceChildren(saved
        ? emptyState(null, null, view.history_search ? 'No matches' : 'Nothing copied yet')
        : emptyState('shieldCheck', 'No Failed Copies', 'Copies with malicious content show up here until Klippy quits.'));
    } else {
      hist.list.replaceChildren(...items.map((item) => row(item, {
        isSelected: item.id === selectedId,
        onTap: () => send(saved ? { SelectItem: item.id } : { SelectFailed: item.id }),
        onCopy: () => send(saved ? { CopyItem: item.id } : { CopyFailedText: item.id }),
      })));
    }
  }

  // Rebuilt only when what it shows changes, so the viewer doesn't reload.
  const detailKey = saved
    ? `saved:${selected ? selected.id : ''}`
    : `failed:${view.selected_failed ? view.selected_failed.id : ''}`;
  if (detailKey === hist.detailKey) return;
  hist.detailKey = detailKey;
  if (saved) {
    hist.detail.replaceChildren(...(selected
      ? savedDetail(selected)
      : [emptyState('clipboard', 'No Selection', 'Select a copied item in the sidebar.')]));
  } else {
    hist.detail.replaceChildren(...(view.selected_failed
      ? failedDetail(view.selected_failed)
      : [emptyState('shieldCheck', 'No Failed Copies', 'Nothing has been blocked this session.')]));
  }
}

function savedDetail(clip) {
  const exportAs = (format) => send({ Export: { id: clip.id, format } });
  const exportButton = h('button', { class: 'button', title: 'Export' }, icon('share'), 'Export');
  const exportMenu = menu(exportButton, FORMATS.map(([format, label]) => ({ label: `${label}…`, action: () => exportAs(format) })));
  return [
    h('div', { class: 'toolbar' },
      h('button', { class: 'button', title: 'Copy', onclick: () => send({ CopyItem: clip.id }) }, icon('copy'), 'Copy'),
      h('div', { class: 'menu-anchor' }, exportButton, exportMenu)),
    h('div', { class: 'viewer' },
      viewerCard(clip),
      h('div', { class: 'export-bar' },
        h('span', { class: 'label' }, icon('share'), 'Export as'),
        FORMATS.map(([format, label, iconName]) => h('button', {
          class: 'button',
          title: `Export as ${label}`,
          onclick: () => exportAs(format),
        }, icon(iconName), label)))),
  ];
}

/**
 * Shows a copy without letting it run anything. HTML goes into a frame with
 * every sandbox permission withheld (no scripts, forms, popups or navigation
 * of this page) and a Content-Security-Policy that blocks all loading; images
 * were cleaned and inlined as data: URIs when it was saved. Each copy is
 * shown light or dark as decided then, whatever the system's mode.
 */
function viewerCard(clip) {
  const theme = { System: '', Light: 'light', Dark: 'dark' }[clip.appearance];
  if (clip.html == null) {
    return h('div', { class: `card ${theme}` }, h('pre', { class: 'selectable' }, clip.text));
  }
  const scheme = { System: 'light dark', Light: 'light', Dark: 'dark' }[clip.appearance];
  const frame = h('iframe', { sandbox: '', referrerPolicy: 'no-referrer', title: 'Copied content' });
  frame.srcdoc =
    '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'; img-src data:">' +
    `<style>:root { color-scheme: ${scheme}; } html { background: Canvas; color: CanvasText; }` +
    ' body { margin: 24px 28px; font: 15px/1.55 "Segoe UI", sans-serif; } img { max-width: 100%; height: auto; }</style>' +
    clip.html;
  return h('div', { class: `card ${theme}` }, frame);
}

/** Why a copy wasn't saved, and its plain text. The HTML itself is never kept or shown. */
function failedDetail(failed) {
  const failedAt = new Date(failed.failed_at_ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
  return [
    h('div', { class: 'toolbar' },
      h('button', { class: 'button', title: 'Copy only the plain text', onclick: () => send({ CopyFailedText: failed.id }) }, icon('copy'), 'Copy Text'),
      h('button', { class: 'button', title: 'Remove from this list', onclick: () => send({ DismissFailed: failed.id }) }, icon('x'), 'Dismiss'),
      h('button', { class: 'button', onclick: () => send('DismissAllFailed') }, 'Dismiss All')),
    h('div', { class: 'failed' },
      h('header', {},
        icon('shield'),
        h('div', {},
          h('h2', {}, 'Not saved: malicious content'),
          h('div', { class: 'secondary' }, `Blocked ${failedAt}. Kept only until Klippy quits.`))),
      h('h3', {}, 'What Klippy found'),
      h('ul', {}, failed.reasons.map((reason) => h('li', {}, icon('x'), reason))),
      h('h3', {}, 'Plain text'),
      h('pre', { class: 'selectable' }, failed.text)),
  ];
}

// MARK: Settings window

const RETENTION_PRESETS = [
  [1, 'Hours', '1 hour'],
  [1, 'Days', '1 day'],
  [7, 'Days', '7 days'],
  [30, 'Days', '30 days'],
  [90, 'Days', '90 days'],
  [365, 'Days', '1 year'],
];

const settings = {};

function renderSettings() {
  const key = JSON.stringify([state.view, state.shell, ui]);
  if (key === settings.key) return;
  settings.key = key;
  const tabs = h('div', { class: 'tabs' },
    h('button', { class: ui.tab === 'general' ? 'tab active' : 'tab', onclick: () => klippy.showTab('general') }, 'General'),
    h('button', { class: ui.tab === 'about' ? 'tab active' : 'tab', onclick: () => klippy.showTab('about') }, 'About'));
  const scroll = root.querySelector('.pane')?.scrollTop ?? 0;
  root.replaceChildren(h('div', { class: 'settings' },
    tabs,
    h('div', { class: 'pane' }, ...(ui.tab === 'about' ? aboutPane() : generalPane(state.view, state.shell)))));
  root.querySelector('.pane').scrollTop = scroll;
}

function setting(label, ...value) {
  return h('div', { class: 'setting' }, h('span', {}, label), h('div', { class: 'value' }, ...value));
}

function select(id, options, value, onchange) {
  return h('select', { id, onchange: (event) => onchange(event.target.value) },
    options.map(([optionValue, label]) => h('option', { value: optionValue, selected: optionValue === value }, label)));
}

function generalPane(view, shell) {
  const current = RETENTION_PRESETS.findIndex(([amount, unit]) => amount === view.retention_amount && unit === view.retention_unit);
  // Chosen "Custom…" this time, so typing a preset's value doesn't snap
  // the menu back to that preset and hide the field.
  const showCustom = ui.customRetention || current < 0;

  const notifications = { true: 'Allowed', false: 'Off', null: 'Not asked yet' }[String(view.notifications_allowed)];

  return [
    h('div', { class: 'section' },
      setting('Launch at login', h('label', { class: 'switch' },
        h('input', {
          type: 'checkbox',
          id: 'launch',
          checked: shell.launch_at_login,
          onchange: (event) => post({ type: 'set_launch_at_login', enabled: event.target.checked }),
        }),
        h('span'))),
      shell.launch_needs_approval && setting('Turned off in Startup apps',
        h('button', { class: 'button', onclick: () => post({ type: 'open_settings_page', page: 'startup_apps' }) }, 'Open Startup Apps…')),
      shell.launch_error && h('div', { class: 'setting error' }, shell.launch_error)),

    h('div', { class: 'section' }, setting('Copy trigger',
      select('trigger', [1, 2, 3, 4].map((count) => [String(count), count === 1 ? '1 time' : `${count} times`]),
        String(view.copy_trigger), (value) => send({ SetCopyTrigger: Number(value) })))),
    h('div', { class: 'footer' }, view.copy_trigger_hint),

    h('div', { class: 'section' },
      setting('Delete items older than',
        select('retention',
          [...RETENTION_PRESETS.map(([, , label], index) => [String(index), label]), ['custom', 'Custom…']],
          showCustom ? 'custom' : String(current),
          (value) => {
            if (value === 'custom') {
              ui.customRetention = true;
              render();
              return;
            }
            ui.customRetention = false;
            const [amount, unit] = RETENTION_PRESETS[Number(value)];
            send({ SetRetention: { amount, unit } });
          })),
      showCustom && setting('Custom',
        h('input', {
          type: 'number',
          id: 'retention-amount',
          min: 1,
          max: view.retention_max_amount,
          value: view.retention_amount,
          title: 'From 1 minute up to 365 days',
          // The core limits it to 1 minute … 365 days, so out-of-range values snap back.
          onchange: (event) => {
            const amount = Number.parseInt(event.target.value, 10);
            if (Number.isNaN(amount)) {
              event.target.value = view.retention_amount;
              return;
            }
            send({ SetRetention: { amount: Math.max(1, Math.min(amount, 4294967295)), unit: view.retention_unit } });
          },
        }),
        select('retention-unit', [['Minutes', 'minutes'], ['Hours', 'hours'], ['Days', 'days']], view.retention_unit,
          (unit) => send({ SetRetention: { amount: view.retention_amount, unit } })))),
    h('div', { class: 'footer' }, view.retention_hint),

    h('div', { class: 'section' }, setting('Quick export format',
      select('quick-export', FORMATS.map(([format, label]) => [format, label]), view.quick_export_format,
        (format) => send({ SetQuickExportFormat: format })))),
    h('div', { class: 'footer' }, "What the export button in the main window's list saves to Downloads."),

    h('div', { class: 'section' }, setting('Clicking a tray history item',
      select('tray-click', [['View', 'Opens it'], ['Copy', 'Copies it to the clipboard']], view.tray_click_action,
        (action) => send({ SetTrayClickAction: action })))),
    h('div', { class: 'footer' }, "What happens when you pick an item in the tray menu's History."),

    h('div', { class: 'section' }, setting('Saved copies',
      h('button', { class: 'button danger', onclick: () => openWindow('clear_history') }, 'Clear History…'))),

    h('div', { class: 'section' }, setting('Notifications',
      h('span', { class: 'secondary' }, notifications),
      view.notifications_allowed === false && h('button', {
        class: 'button',
        onclick: () => post({ type: 'open_settings_page', page: 'notifications' }),
      }, 'Open Settings…'))),
  ];
}

function aboutPane() {
  const link = (label, target) => h('button', { class: 'link', onclick: () => post({ type: 'open_link', link: target }) }, label);
  return [
    h('div', { class: 'section about-header' }, appIcon(), h('h2', {}, 'Klippy')),
    h('div', { class: 'section' }, setting('Version', h('span', { class: 'secondary' }, document.body.dataset.version))),
    h('div', { class: 'section' },
      setting('Made by', h('span', { class: 'secondary' }, 'Xplorebits Technologies')),
      setting('Website', link('xplorebits.com', 'website'))),
    // Required by the icon's CC BY 4.0 licence.
    h('div', { class: 'footer' }, 'Logo'),
    h('div', { class: 'section' },
      setting('Icon', h('span', { class: 'secondary' }, 'solar:paperclip-rounded-2-linear')),
      setting('From', link('Solar Icons Set by 480 Design', 'icons')),
      setting('Licence', link('CC BY 4.0', 'licence'))),
  ];
}

// MARK: Clear History window

const CONFIRMATION = 'confirm';
const clear = {};

function renderClearHistory() {
  if (!clear.input) {
    clear.input = h('input', {
      type: 'text',
      placeholder: CONFIRMATION,
      spellcheck: false,
      oninput: renderClearHistory,
      onkeydown: (event) => {
        if (event.key === 'Enter') deleteIfConfirmed();
      },
    });
    clear.button = h('button', { class: 'button danger primary', onclick: deleteIfConfirmed }, 'Delete All');
    root.replaceChildren(h('div', { class: 'dialog' },
      h('header', {},
        icon('trash'),
        h('div', {},
          h('h2', {}, 'Clear all history?'),
          h('p', { class: 'secondary' }, "Every saved copy is permanently deleted. This can't be undone."))),
      h('label', {}, h('span', {}, 'Type ', h('b', {}, CONFIRMATION), ' to continue'), clear.input),
      h('div', { class: 'buttons' }, h('button', { class: 'button', onclick: closeWindow }, 'Cancel'), clear.button)));
    clear.input.focus();
  }
  clear.button.disabled = !isConfirmed();
}

function isConfirmed() {
  return clear.input.value.trim() === CONFIRMATION;
}

function deleteIfConfirmed() {
  if (!isConfirmed()) return;
  send('ClearHistory');
  closeWindow();
}

// MARK: Setup

const renderers = {
  main: renderMain,
  history: renderHistory,
  settings: renderSettings,
  clear_history: renderClearHistory,
};

function render() {
  if (!state) return;
  // Settings rebuilds its controls; keep the focused one focused.
  const focused = document.activeElement?.id;
  renderers[windowKind]();
  if (focused && document.activeElement?.id !== focused) document.getElementById(focused)?.focus();
}

document.addEventListener('click', closeMenus);

document.addEventListener('contextmenu', (event) => {
  if (!event.target.closest('input, .selectable')) event.preventDefault();
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    if (document.querySelector('.menu:not(.hidden)')) closeMenus();
    else if (windowKind === 'clear_history') closeWindow();
    return;
  }
  if (!event.ctrlKey || event.altKey) return;
  const key = event.key.toLowerCase();
  if (key === ',') openWindow('settings', 'general');
  else if (key === 'q') post({ type: 'quit' });
  else if (key === 'w') closeWindow();
  else if (key === 'f') root.querySelector('input[type="search"]')?.focus();
  else return;
  event.preventDefault();
});

post({ type: 'ready' });
