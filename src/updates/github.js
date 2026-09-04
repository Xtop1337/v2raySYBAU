import crypto from 'node:crypto';

export class GitHubUpdater {
  constructor({ owner, repo, currentVersion, fetchImpl = fetch, notify = () => {}, verifySignature, launchInstaller }) {
    Object.assign(this, { owner, repo, currentVersion, fetchImpl, notify, verifySignature, launchInstaller });
  }

  async latestRelease() {
    const response = await this.fetchImpl(`https://api.github.com/repos/${this.owner}/${this.repo}/releases/latest`, { headers: { Accept: 'application/vnd.github+json' } });
    if (!response.ok) throw new Error(`GitHub release check failed: HTTP ${response.status}`);
    const release = await response.json();
    return { version: release.tag_name.replace(/^v/, ''), notes: release.body ?? '', assets: release.assets ?? [], url: release.html_url };
  }

  async check() {
    const release = await this.latestRelease();
    const available = compareVersions(release.version, this.currentVersion) > 0;
    if (available) this.notify({ type: 'update-available', release });
    return { available, release };
  }

  async downloadAndInstall(asset, { expectedSha256, signature } = {}) {
    if (!asset?.browser_download_url || !expectedSha256 || !signature) throw new Error('Asset URL, SHA-256 and signature are required');
    const response = await this.fetchImpl(asset.browser_download_url);
    if (!response.ok) throw new Error(`Artifact download failed: HTTP ${response.status}`);
    const artifact = Buffer.from(await response.arrayBuffer());
    const actual = crypto.createHash('sha256').update(artifact).digest('hex');
    const expected = expectedSha256.toLowerCase();
    if (!/^[a-f0-9]{64}$/.test(expected) || !crypto.timingSafeEqual(Buffer.from(actual), Buffer.from(expected))) throw new Error('Artifact checksum verification failed');
    if (!this.verifySignature || !(await this.verifySignature(artifact, signature))) throw new Error('Artifact signature verification failed');
    this.notify({ type: 'update-verified', asset: asset.name });
    await this.launchInstaller(artifact, asset.name);
  }

  /** Downloads a GitHub-hosted subscription and delegates an atomic replacement to the host app. */
  async updateSubscription(asset, { expectedSha256, signature, replaceAtomically }) {
    if (typeof replaceAtomically !== 'function') throw new Error('Atomic subscription replacement callback is required');
    const verified = new GitHubUpdater({ ...this, launchInstaller: async (artifact) => replaceAtomically(artifact) });
    await verified.downloadAndInstall(asset, { expectedSha256, signature });
    this.notify({ type: 'subscription-updated', asset: asset.name });
  }
}

export function compareVersions(left, right) {
  const a = left.split('.').map(Number); const b = right.split('.').map(Number);
  for (let index = 0; index < Math.max(a.length, b.length); index += 1) {
    const difference = (a[index] ?? 0) - (b[index] ?? 0);
    if (difference) return Math.sign(difference);
  }
  return 0;
}
