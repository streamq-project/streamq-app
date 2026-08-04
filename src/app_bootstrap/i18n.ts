import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'

import en from './locales/en.yml'
import ru from './locales/ru.yml'

const config = window.appAPI.bootstrap.init()

const SUPPORTED_LANGUAGES = ['en', 'ru'] as const

i18n
  .use(initReactI18next)
  .init({
    supportedLngs: SUPPORTED_LANGUAGES,
    fallbackLng: 'en',
    ns: ['common'],
    defaultNS: 'common',
    interpolation: { escapeValue: false },
    resources: { en: { common: en }, ru: { common: ru } },
    lng: config.settings.language ?? navigator.language.slice(0, 2),
    react: { bindI18n: 'loaded languageChanged' },
    nsSeparator: '>'
  })

export default i18n
