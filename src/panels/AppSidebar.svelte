<script lang="ts">
  import FileTree from "../components/files/FileTree.svelte";
  import ItemList, { type Item } from "../components/ui/ItemList.svelte";
  import type { FileEntry, GitStatus, SearchHit } from "../types";

  export let workspace = "";
  export let workspaceTitle = "workspace";
  export let showWorkspace: () => void = () => {};
  export let sideTab: "files" | "content" | "git" = "files";
  export let setSideTab: (tab: "files" | "content" | "git") => void = () => {};
  export let featureContentSearch = true;
  export let featureGit = true;
  export let fileQuery = "";
  export let inputFileQuery: (event: Event) => void = () => {};
  export let fileSearching = false;
  export let fileSearchTruncated = false;
  export let fileTreeKey = "";
  export let visibleFiles: FileEntry[] = [];
  export let expandedFilePaths: string[] = [];
  export let openFile: (entry: FileEntry) => void = () => {};
  export let contentQuery = "";
  export let setContentQuery: (value: string) => void = () => {};
  export let contentSearchKeydown: (event: KeyboardEvent) => void = () => {};
  export let contentSearching = false;
  export let runContentSearch: () => void = () => {};
  export let contentResults: SearchHit[] = [];
  export let openSearchHit: (hit: SearchHit) => void = () => {};
  export let fileName: (path: string) => string = (path) => path;
  export let gitLoading = false;
  export let refreshGit: () => void = () => {};
  export let openGitDiff: (path?: string) => void = () => {};
  export let gitStatus: GitStatus | null = null;
  export let sessionQuery = "";
  export let setSessionQuery: (value: string) => void = () => {};
  export let sessionItems: Item[] = [];
  export let newSession: () => void = () => {};
</script>

<aside class="sidebar">
  <section class="side-section workspace-section">
    <div class="side-title">workspace</div>
    <button class="ghost workspace-button" type="button" title={workspace} on:click={showWorkspace}>{workspaceTitle}</button>
  </section>

  <section class="side-section files-section">
    <div class="side-tabs">
      <button class:active={sideTab === "files"} type="button" on:click={() => setSideTab("files")}>files</button>
      {#if featureContentSearch}<button class:active={sideTab === "content"} type="button" on:click={() => setSideTab("content")}>content</button>{/if}
      {#if featureGit}<button class:active={sideTab === "git"} type="button" on:click={() => setSideTab("git")}>git</button>{/if}
    </div>

    {#if sideTab === "files"}
      <input class="side-search" value={fileQuery} on:input={inputFileQuery} placeholder="search" />
      {#if fileSearching}<span class="empty-state">searching...</span>{/if}
      {#if fileSearchTruncated}<span class="empty-state">showing first 500 matches</span>{/if}
      {#key fileTreeKey}
        <FileTree entries={visibleFiles} expandedPaths={expandedFilePaths} onOpen={openFile} />
      {/key}
    {:else if sideTab === "content" && featureContentSearch}
      <div class="inline-row">
        <input value={contentQuery} placeholder="rg search" on:input={(event) => setContentQuery((event.currentTarget as HTMLInputElement).value)} on:keydown={contentSearchKeydown} />
        <button class="ghost compact" type="button" disabled={contentSearching} on:click={runContentSearch}>go</button>
      </div>
      <div class="compact-list">
        {#each contentResults as hit (`${hit.path}:${hit.line}:${hit.column}`)}
          <button class="content-result" type="button" title={`${hit.path}:${hit.line}:${hit.column}\n${hit.text}`} on:click={() => openSearchHit(hit)}>
            <div class="result-file">
              <span>{fileName(hit.path)}</span>
              <small>L{hit.line}:C{hit.column}</small>
            </div>
            <div class="result-line">{hit.text.trim() || " "}</div>
          </button>
        {:else}
          <span class="empty-state">{contentQuery.trim() ? "no matches found" : "type query + press Enter"}</span>
        {/each}
      </div>
    {:else if sideTab === "git" && featureGit}
      <div class="inline-row">
        <button class="ghost compact" type="button" disabled={gitLoading} on:click={refreshGit}>status</button>
        <button class="ghost compact" type="button" disabled={gitLoading} on:click={() => openGitDiff()}>diff</button>
      </div>
      {#if gitStatus}
        <div class="hint">{gitStatus.branch} · {gitStatus.entries.length} changed</div>
        <div class="compact-list">
          {#each gitStatus.entries as entry (entry.raw)}
            <button class="result-row" type="button" title={entry.raw} on:click={() => openGitDiff(entry.path)}>
              <strong>{entry.status} {entry.path}</strong>
            </button>
          {:else}
            <span class="empty-state">working tree clean</span>
          {/each}
        </div>
      {/if}
    {/if}
  </section>

  <section class="side-section sessions-section">
    <div class="side-title">sessions</div>
    <input value={sessionQuery} placeholder="search" on:input={(event) => setSessionQuery((event.currentTarget as HTMLInputElement).value)} />
    <ItemList items={sessionItems} addTitle="+ new session" addSubtitle="empty chat" onAdd={newSession} />
  </section>
</aside>
