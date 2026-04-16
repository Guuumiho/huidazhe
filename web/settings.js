import { THEME_PRESETS, state } from './state.js';
import { els, invoke, setConfigStatus, setFormMessage } from './ui.js';

export function collectSettings() {
  return {
    apiUrl: els.apiUrl.value.trim(),
    apiKey: els.apiKey.value.trim(),
    theme: els.themeSelect.value,
    lastConversationId: state.currentConversationId,
  };
}

export function applyTheme(themeKey) {
  const theme = THEME_PRESETS[themeKey] || THEME_PRESETS['default-theme'];
  const root = document.documentElement;

  root.style.setProperty('--assistant-bubble', theme.assistantBubble);
  root.style.setProperty('--user-bubble', theme.userBubble);
  root.style.setProperty('--markdown-h1-size', theme.h1Size);
  root.style.setProperty('--markdown-h1-weight', theme.h1Weight);
  root.style.setProperty('--markdown-h1-color', theme.h1Color);
  root.style.setProperty('--markdown-h2-size', theme.h2Size);
  root.style.setProperty('--markdown-h2-weight', theme.h2Weight);
  root.style.setProperty('--markdown-h2-color', theme.h2Color);
  root.style.setProperty('--markdown-h3-size', theme.h3Size);
  root.style.setProperty('--markdown-h3-weight', theme.h3Weight);
  root.style.setProperty('--markdown-h3-color', theme.h3Color);
  root.style.setProperty('--markdown-body-size', theme.bodySize);
  root.style.setProperty('--markdown-body-weight', theme.bodyWeight);
  root.style.setProperty('--markdown-body-color', theme.bodyColor);
  root.style.setProperty('--markdown-strong-size', theme.strongSize);
  root.style.setProperty('--markdown-strong-weight', theme.strongWeight);
  root.style.setProperty('--markdown-strong-color', theme.strongColor);
  root.style.setProperty('--markdown-divider-color', theme.dividerColor);
}

export function applySettings(settings) {
  els.apiUrl.value = settings.apiUrl || '';
  els.apiKey.value = settings.apiKey || '';
  els.themeSelect.value = THEME_PRESETS[settings.theme] ? settings.theme : 'default-theme';
  state.lastConversationId = settings.lastConversationId ?? null;
  applyTheme(els.themeSelect.value);

  if (settings.apiUrl && settings.apiKey) {
    setConfigStatus(true, 'API ready');
  } else {
    setConfigStatus(false, 'API not set');
  }
}

export async function loadSettings() {
  const settings = await invoke('load_settings');
  applySettings(settings);
  return settings;
}

export async function saveSettings(showMessage = true) {
  const saved = await invoke('save_settings', { settings: collectSettings() });
  applySettings(saved);
  if (showMessage) {
    setFormMessage('配置已保存', 'success');
  }
  return saved;
}

export function bindSettingsEvents(onThemeChange) {
  els.toggleSettings.addEventListener('click', () => {
    els.settingsPanel.classList.toggle('hidden');
  });

  els.saveSettings.addEventListener('click', async () => {
    setFormMessage('');
    try {
      await saveSettings(true);
    } catch (error) {
      setFormMessage(String(error), 'error');
    }
  });

  els.themeSelect.addEventListener('change', () => {
    applyTheme(els.themeSelect.value);
    onThemeChange?.();
  });
}
