// Source history owns this date; a build must never fabricate a release date.
export function sourceUpdateDate(changelog) {
  const body = changelog.match(/^## \[Unreleased\][^\S\n]*\n([\s\S]*?)(?=^## |(?![\s\S]))/m)?.[1];
  if (!body || !/^- /m.test(body)) throw new Error('Expected a nonempty Unreleased source section.');
  const date = body.match(/<!-- unreleased-updated: (\d{4}-\d{2}-\d{2}) -->/)?.[1];
  const parsed = date ? new Date(`${date}T00:00:00Z`) : null;
  if (!date || !parsed || Number.isNaN(parsed.valueOf()) || parsed.toISOString().slice(0,10) !== date) {
    throw new Error('A nonempty Unreleased section requires a valid source update date.');
  }
  return date;
}
export function formatUpdateDate(date, locale) {
  return new Intl.DateTimeFormat(locale === 'fr' ? 'fr-FR' : 'en-GB', {
    day:'numeric', month:'long', year:'numeric', timeZone:'UTC',
  }).format(new Date(`${date}T00:00:00Z`));
}
