<script lang="ts">
  import { api } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import { errorText, toast } from '../lib/store.svelte';
  import type { Profile } from '../lib/types';
  import Modal from './Modal.svelte';
  import Toggle from './Toggle.svelte';

  let { profile, isNew, onclose }: { profile: Profile; isNew: boolean; onclose: () => void } = $props();

  // Edit a deep copy; nothing changes until "Save".
  // svelte-ignore state_referenced_locally
  let p = $state(structuredClone($state.snapshot(profile)) as Profile);
  let alpn = $state(p.tls.alpn.join(','));
  if (p.transport.mode === 'auto') p.transport.mode = '';

  const xrayProtocol = $derived(p.protocol === 'vless' || p.protocol === 'vmess' || p.protocol === 'trojan');
  const usesUuid = $derived(p.protocol === 'vless' || p.protocol === 'vmess' || p.protocol === 'tuic');
  const usesPassword = $derived(p.protocol !== 'vless' && p.protocol !== 'vmess');

  const ssMethods = [
    '2022-blake3-aes-128-gcm',
    '2022-blake3-aes-256-gcm',
    '2022-blake3-chacha20-poly1305',
    'aes-128-gcm',
    'aes-256-gcm',
    'chacha20-ietf-poly1305',
    'xchacha20-ietf-poly1305',
  ];

  async function submit() {
    p.tls.alpn = alpn
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    p.port = Number(p.port) || 443;
    p.alterId = Number(p.alterId) || 0;
    p.quic.upMbps = Number(p.quic.upMbps) || 0;
    p.quic.downMbps = Number(p.quic.downMbps) || 0;
    if (!p.address.trim()) {
      toast(`${t('editor.address')}?`, 'error');
      return;
    }
    try {
      await api.saveProfile($state.snapshot(p) as Profile);
      onclose();
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }
</script>

<Modal title={isNew ? t('editor.title.new') : t('editor.title.edit')} width={680} {onclose}>
  <div class="section-title">{t('editor.basic')}</div>
  <div class="grid">
    <label class="wide"><span>{t('editor.name')}</span><input class="input" bind:value={p.name} /></label>
    <label><span>{t('editor.protocol')}</span>
      <select class="select" bind:value={p.protocol}>
        <option value="vless">VLESS</option>
        <option value="vmess">VMess</option>
        <option value="trojan">Trojan</option>
        <option value="shadowsocks">Shadowsocks</option>
        <option value="hysteria2">Hysteria2</option>
        <option value="tuic">TUIC</option>
      </select>
    </label>
    <label><span>{t('editor.group')}</span><input class="input" bind:value={p.group} /></label>
    <label class="wide"><span>{t('editor.address')}</span><input class="input" bind:value={p.address} placeholder="example.com" /></label>
    <label><span>{t('editor.port')}</span><input class="input" type="number" min="1" max="65535" bind:value={p.port} /></label>
    {#if usesUuid}
      <label class="wide"><span>{t('editor.uuid')}</span><input class="input mono" bind:value={p.uuid} /></label>
    {/if}
    {#if usesPassword}
      <label class="wide"><span>{t('editor.password')}</span><input class="input mono" bind:value={p.password} /></label>
    {/if}
    {#if p.protocol === 'vless'}
      <label><span>{t('editor.flow')}</span>
        <select class="select" bind:value={p.flow}>
          <option value="">—</option>
          <option value="xtls-rprx-vision">xtls-rprx-vision</option>
          <option value="xtls-rprx-vision-udp443">xtls-rprx-vision-udp443</option>
        </select>
      </label>
      <label><span>{t('editor.method')}</span><input class="input" bind:value={p.method} placeholder="none" /></label>
    {:else if p.protocol === 'vmess'}
      <label><span>{t('editor.method')}</span>
        <select class="select" bind:value={p.method}>
          {#each ['auto', 'aes-128-gcm', 'chacha20-poly1305', 'none', 'zero'] as m}<option value={m}>{m}</option>{/each}
        </select>
      </label>
      <label><span>{t('editor.alterId')}</span><input class="input" type="number" bind:value={p.alterId} /></label>
    {:else if p.protocol === 'shadowsocks'}
      <label class="wide"><span>{t('editor.method')}</span>
        <select class="select" bind:value={p.method}>
          {#each ssMethods as m}<option value={m}>{m}</option>{/each}
        </select>
      </label>
    {/if}
    <label class="wide"><span>{t('editor.dns')}</span><input class="input" bind:value={p.dns} placeholder="https://1.1.1.1/dns-query" /></label>
  </div>

  {#if xrayProtocol}
    <div class="section-title">{t('editor.transport')}</div>
    <div class="grid">
      <label><span>{t('editor.network')}</span>
        <select class="select" bind:value={p.transport.network}>
          <option value="tcp">TCP (raw)</option>
          <option value="ws">WebSocket</option>
          <option value="grpc">gRPC</option>
          <option value="xhttp">xHTTP</option>
          <option value="httpupgrade">HTTPUpgrade</option>
          <option value="kcp">mKCP</option>
        </select>
      </label>
      {#if p.transport.network === 'tcp'}
        <label><span>{t('editor.headerType')}</span>
          <select class="select" bind:value={p.transport.headerType}>
            <option value="">none</option>
            <option value="http">http</option>
          </select>
        </label>
      {/if}
      {#if p.transport.network === 'grpc'}
        <label><span>{t('editor.serviceName')}</span><input class="input" bind:value={p.transport.serviceName} /></label>
        <label><span>{t('editor.mode')}</span>
          <select class="select" bind:value={p.transport.mode}>
            <option value="">gun</option>
            <option value="multi">multi</option>
          </select>
        </label>
      {/if}
      {#if p.transport.network === 'xhttp'}
        <label><span>{t('editor.mode')}</span>
          <select class="select" bind:value={p.transport.mode}>
            <option value="">auto</option>
            {#each ['packet-up', 'stream-up', 'stream-one'] as m}<option value={m}>{m}</option>{/each}
          </select>
        </label>
      {/if}
      {#if p.transport.network !== 'grpc' && p.transport.network !== 'kcp'}
        <label><span>{t('editor.host')}</span><input class="input" bind:value={p.transport.host} /></label>
        <label><span>{t('editor.path')}</span><input class="input" bind:value={p.transport.path} placeholder="/" /></label>
      {/if}
      {#if p.transport.network === 'xhttp'}
        <label class="full"><span>{t('editor.extra')}</span><textarea class="textarea" rows="3" bind:value={p.transport.extra}></textarea></label>
      {/if}
    </div>
  {/if}

  {#if p.protocol !== 'shadowsocks'}
    <div class="section-title">{t('editor.security')}</div>
    <div class="grid">
      {#if xrayProtocol}
        <label><span>{t('editor.securityType')}</span>
          <select class="select" bind:value={p.tls.security}>
            <option value="none">none</option>
            <option value="tls">TLS</option>
            {#if p.protocol !== 'vmess'}<option value="reality">Reality</option>{/if}
          </select>
        </label>
      {/if}
      {#if p.tls.security !== 'none' || !xrayProtocol}
        <label><span>{t('editor.sni')}</span><input class="input" bind:value={p.tls.sni} /></label>
        {#if xrayProtocol}
          <label><span>{t('editor.fingerprint')}</span>
            <select class="select" bind:value={p.tls.fingerprint}>
              {#each ['chrome', 'firefox', 'safari', 'edge', 'ios', 'android', 'random', 'randomized', 'qq', '360'] as f}<option value={f}>{f}</option>{/each}
            </select>
          </label>
        {/if}
        {#if p.tls.security === 'reality'}
          <label class="wide"><span>{t('editor.publicKey')}</span><input class="input mono" bind:value={p.tls.publicKey} /></label>
          <label><span>{t('editor.shortId')}</span><input class="input mono" bind:value={p.tls.shortId} /></label>
          <label><span>{t('editor.spiderX')}</span><input class="input" bind:value={p.tls.spiderX} placeholder="/" /></label>
        {:else}
          <label><span>{t('editor.alpn')}</span><input class="input" bind:value={alpn} placeholder="h2,http/1.1" /></label>
          <div class="full"><Toggle bind:checked={p.tls.allowInsecure} label={t('editor.allowInsecure')} /></div>
        {/if}
      {/if}
      {#if p.protocol === 'hysteria2'}
        <label><span>{t('editor.obfs')}</span>
          <select class="select" bind:value={p.quic.obfs}>
            <option value="">—</option>
            <option value="salamander">salamander</option>
          </select>
        </label>
        {#if p.quic.obfs}<label><span>{t('editor.obfsPassword')}</span><input class="input" bind:value={p.quic.obfsPassword} /></label>{/if}
        <label><span>{t('editor.hopPorts')}</span><input class="input" bind:value={p.quic.hopPorts} placeholder="20000-30000" /></label>
        <label><span>{t('editor.upMbps')}</span><input class="input" type="number" bind:value={p.quic.upMbps} /></label>
        <label><span>{t('editor.downMbps')}</span><input class="input" type="number" bind:value={p.quic.downMbps} /></label>
      {/if}
      {#if p.protocol === 'tuic'}
        <label><span>{t('editor.congestion')}</span>
          <select class="select" bind:value={p.quic.congestionControl}>
            {#each ['bbr', 'cubic', 'new_reno'] as c}<option value={c}>{c}</option>{/each}
          </select>
        </label>
        <label><span>{t('editor.udpRelay')}</span>
          <select class="select" bind:value={p.quic.udpRelayMode}>
            {#each ['native', 'quic'] as c}<option value={c}>{c}</option>{/each}
          </select>
        </label>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <button class="btn" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn primary" onclick={submit}>{t('common.save')}</button>
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px 12px;
    margin-bottom: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
    color: var(--text-2);
    grid-column: span 2;
  }

  .wide {
    grid-column: span 2;
  }

  .full {
    grid-column: 1 / -1;
  }
</style>
