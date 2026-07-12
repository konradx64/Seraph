<script>
  import { Key, Plus, Zap } from '@lucide/svelte';

  // Svelte 5 props
  let { certs = [], onRegister, onRefresh, onGenerate } = $props();

  // Local form inputs state
  let newSni = $state("");
  let newCertPem = $state("");
  let newKeyPem = $state("");
  let generateDomain = $state("");

  function handleSubmit() {
    if (!newSni || !newCertPem || !newKeyPem) return;
    onRegister(newSni, newCertPem, newKeyPem);
    newSni = "";
    newCertPem = "";
    newKeyPem = "";
  }

  function handleGenerate() {
    if (!generateDomain) return;
    onGenerate(generateDomain);
    generateDomain = "";
  }
</script>

<div class="grid grid-cols-1 lg:grid-cols-3 gap-6 animate-fade-in">
  <!-- Certs Table -->
  <div class="card lg:col-span-2 bg-white border border-slate-200/80 rounded-xl shadow-xs">
    <div class="card-body p-6">
      <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
        <Key class="w-4 h-4 text-cyan-500" />
        Active SSL Certificates
      </h2>
      <div class="overflow-x-auto">
        <table class="table table-sm w-full">
          <thead>
            <tr class="text-slate-400 border-slate-100">
              <th>Domain Name (SNI)</th>
              <th>Status</th>
              <th class="w-24 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="text-slate-700 text-sm">
            {#each certs as cert}
              <tr class="border-slate-100 hover:bg-slate-50/50">
                <td class="font-mono text-xs font-semibold text-slate-800">{cert}</td>
                <td>
                  <span class="badge badge-sm text-emerald-700 border border-emerald-200/60 font-bold text-[10px]">Active</span>
                </td>
                <td class="text-right">
                  <button class="btn btn-xs btn-outline rounded-md border-slate-200 hover:border-cyan-400 hover:bg-cyan-50/50 hover:text-cyan-700 font-bold text-[10px]" 
                    onclick={() => onRefresh(cert)}>
                    Refresh
                  </button>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="3" class="text-center py-12 text-slate-400 text-sm">No SSL certificates registered in database.</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  </div>

  <!-- Sidebar -->
  <div class="space-y-6">
    <!-- Quick Generate Card -->
    <div class="card bg-white border border-slate-200/80 rounded-xl shadow-xs">
      <div class="card-body p-6">
        <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
          <Zap class="w-4 h-4 text-amber-500" />
          Generate Certificate
        </h2>
        <p class="text-slate-500 text-xs mb-3">Create a self-signed certificate instantly for development or internal use.</p>
        <form onsubmit={(e) => { e.preventDefault(); handleGenerate(); }} class="space-y-3">
          <div class="form-control">
            <input id="gen-domain-input" type="text" placeholder="e.g. myapp.local" class="input input-bordered w-full input-sm rounded-md focus:border-amber-500 focus:outline-hidden" bind:value={generateDomain} required />
          </div>
          <button type="submit" class="btn btn-sm w-full bg-amber-500 hover:bg-amber-600 border-none rounded-md font-bold text-white shadow-xs">
            Generate Self-Signed
          </button>
        </form>
      </div>
    </div>

    <!-- Manual Upload Card -->
    <div class="card bg-white border border-slate-200/80 rounded-xl shadow-xs">
      <div class="card-body p-6">
        <h2 class="text-slate-900 font-bold text-base mb-4 flex items-center gap-2">
          <Plus class="w-4 h-4 text-cyan-500" />
          Upload Certificate
        </h2>
        <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-4">
          <div class="form-control">
            <label class="label py-1" for="cert-sni-input">
              <span class="label-text text-slate-600 font-bold text-xs">Domain Name (SNI)</span>
            </label>
            <input id="cert-sni-input" type="text" placeholder="e.g. app.localhost" class="input input-bordered w-full input-sm rounded-md focus:border-cyan-500 focus:outline-hidden" bind:value={newSni} required />
          </div>

          <div class="form-control">
            <label class="label py-1" for="cert-pem-input">
              <span class="label-text text-slate-600 font-bold text-xs">Certificate PEM Chain</span>
            </label>
            <textarea id="cert-pem-input" rows="3" placeholder="-----BEGIN CERTIFICATE-----..." class="textarea textarea-bordered w-full text-xs font-mono rounded-md focus:border-cyan-500 focus:outline-hidden" bind:value={newCertPem} required></textarea>
          </div>

          <div class="form-control">
            <label class="label py-1" for="cert-key-input">
              <span class="label-text text-slate-600 font-bold text-xs">Private Key PEM</span>
            </label>
            <textarea id="cert-key-input" rows="3" placeholder="-----BEGIN PRIVATE KEY-----..." class="textarea textarea-bordered w-full text-xs font-mono rounded-md focus:border-cyan-500 focus:outline-hidden" bind:value={newKeyPem} required></textarea>
          </div>

          <button type="submit" class="btn btn-sm w-full mt-4 bg-cyan-500 hover:bg-cyan-600 border-none rounded-md font-bold text-white shadow-xs">
            Save Certificate
          </button>
        </form>
      </div>
    </div>
  </div>
</div>
