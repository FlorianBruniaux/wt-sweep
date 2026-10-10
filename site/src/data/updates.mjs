// Source history owns this date; a build must never fabricate a release date.
export function sourceUpdateDate(changelog) {
  const body = changelog.match(/^## \[Unreleased\][^\S\n]*\n([\s\S]*?)(?=^## |(?![\s\S]))/m)?.[1];
  const date = body && /^- /m.test(body)
    ? body.match(/<!-- unreleased-updated: (\d{4}-\d{2}-\d{2}) -->/)?.[1]
    : changelog.match(/^## \[\d+\.\d+\.\d+(?:-[\w.-]+)?\] - (\d{4}-\d{2}-\d{2})$/m)?.[1];
  if (!body && !date) throw new Error('Expected a nonempty Unreleased source section or a dated release.');
  const parsed = date ? new Date(`${date}T00:00:00Z`) : null;
  if (!date || !parsed || Number.isNaN(parsed.valueOf()) || parsed.toISOString().slice(0,10) !== date) {
    throw new Error('Changelog requires a valid source update date.');
  }
  return date;
}
export function formatUpdateDate(date, locale) {
  return new Intl.DateTimeFormat(locale === 'fr' ? 'fr-FR' : 'en-GB', {
    day:'numeric', month:'long', year:'numeric', timeZone:'UTC',
  }).format(new Date(`${date}T00:00:00Z`));
}
