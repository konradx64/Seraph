<script>
  import { Terminal } from '@lucide/svelte';

  // Svelte 5 props
  let { logs = $bindable([]) } = $props();

  function clearLogs() {
    logs = [];
  }
</script>

<div class="card bg-white border border-slate-200/80 rounded-xl shadow-xs animate-fade-in">
  <div class="card-body p-6">
    <div class="flex justify-between items-center mb-4">
      <h2 class="text-slate-900 font-bold text-base flex items-center gap-2">
        <Terminal class="w-4 h-4 text-cyan-500" />
        Live Event Stream
      </h2>
      <button class="btn btn-xs btn-outline rounded-md text-slate-500 border-slate-200 hover:bg-slate-50 hover:text-slate-700" 
        onclick={clearLogs}>
        Clear Console
      </button>
    </div>
    <div class="bg-slate-950 text-emerald-400 font-mono text-xs p-5 rounded-xl h-[450px] overflow-y-auto space-y-1.5 shadow-inner">
      {#each logs as log}
        <div>
          <span class="text-slate-500">[{log.time}]</span> {log.text}
        </div>
      {:else}
        <div class="text-slate-600 italic">Listening for proxy hits and routing updates...</div>
      {/each}
    </div>
  </div>
</div>
