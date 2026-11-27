<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { type EngineStatus, statusLine } from './health.ts';

  // M0 placeholder (PLAN §7 task 5): one status line from GET /api/v1/health, polled once a
  // second. The real UI is built to Usopp's approved mockups from M4 on.
  let status = $state<EngineStatus | null>(null);
  let failure = $state<string | null>(null);

  async function poll() {
    try {
      status = await invoke<EngineStatus>('engine_status');
      failure = null;
    } catch (e) {
      failure = String(e);
    }
  }

  onMount(() => {
    poll();
    const timer = setInterval(poll, 1000);
    return () => clearInterval(timer);
  });

  const detail = $derived(
    failure ?? status?.error ?? status?.health?.renderer.detail ?? null,
  );
</script>

<main>
  <h1>noPlayout</h1>
  <p class="status" data-testid="status">
    {status ? statusLine(status) : 'Engine: connecting…'}
  </p>
  {#if detail}
    <p class="detail">{detail}</p>
  {/if}
  {#if status?.health}
    <p class="meta">
      Engine {status.health.engine.version} on port {status.port}
      ({status.mode === 'attached' ? 'already running' : 'started by this app'})
      {#if status.health.renderer.chromium_version}
        · Chromium {status.health.renderer.chromium_version}
      {/if}
    </p>
  {/if}
</main>

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: #16191f;
    color: #e8eaee;
    font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
  }
  main {
    padding: 32px 40px;
  }
  h1 {
    margin: 0 0 24px;
    font-size: 22px;
    font-weight: 600;
  }
  .status {
    font-size: 20px;
    margin: 0 0 12px;
  }
  .detail {
    color: #ffb4a8;
    margin: 0 0 12px;
  }
  .meta {
    color: #9aa3b2;
    font-size: 13px;
    margin: 0;
  }
</style>
