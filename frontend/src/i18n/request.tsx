import { getRequestConfig } from 'next-intl/server';
import enTranslations from '@/locales/en.json';
import esTranslations from '@/locales/es.json';

export default getRequestConfig(async ({ locale }) => ({
  messages: locale === 'es' ? esTranslations : enTranslations,
}));
