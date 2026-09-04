const DOMAIN_PATTERN = /^(\*\.)?([a-z0-9-]+\.)+[a-z]{2,}$/i;

export function validateRouting(settings = {}) {
  const rules = settings.rules ?? [];
  const applications = settings.applications ?? [];
  for (const rule of rules) {
    if (!DOMAIN_PATTERN.test(rule.domain)) throw new Error(`Invalid routing domain: ${rule.domain}`);
    if (!['proxy', 'direct', 'block'].includes(rule.action)) throw new Error(`Invalid routing action: ${rule.action}`);
  }
  if (applications.some((app) => typeof app !== 'string' || !app.trim())) throw new Error('Application rules must contain executable names');
  return { rules, applications: [...new Set(applications)], proxySelectedApplicationsOnly: Boolean(settings.proxySelectedApplicationsOnly) };
}

export function resolveRoute(hostname, executable, settings) {
  const config = validateRouting(settings);
  const appAllowed = !config.proxySelectedApplicationsOnly || config.applications.includes(executable);
  const matching = config.rules.find(({ domain }) => hostname === domain.replace(/^\*\./, '') || (domain.startsWith('*.') && hostname.endsWith(domain.slice(1))));
  if (matching) return matching.action;
  return appAllowed ? 'proxy' : 'direct';
}
