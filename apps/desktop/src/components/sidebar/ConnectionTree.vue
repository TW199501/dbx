<script setup lang="ts">
import { ref, computed, nextTick, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Search, X, ListFilter, Check, FolderPlus } from "lucide-vue-next";
import { useConnectionStore } from "@/stores/connectionStore";
import { useQueryStore } from "@/stores/queryStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { TreeNode, TreeNodeType } from "@/types/database";
import { filterSidebarTree } from "@/lib/sidebarSearchTree";
import { isCancelSearchShortcut } from "@/lib/keyboardShortcuts";
import { findSidebarNodeForActiveTab, scrollTopForSidebarNode } from "@/lib/sidebarActiveTabTarget";
import {
  SIDEBAR_TREE_ROW_HEIGHT,
  SIDEBAR_TREE_PRERENDER_COUNT,
  SIDEBAR_TREE_SCROLL_BUFFER,
  flattenTree,
  scrollTopForExpandedTreeNode,
  shouldAutoScrollExpandedTreeNode,
  shouldVirtualizeFlatTree,
  type FlatTreeNode,
} from "@/composables/useFlatTree";
import TreeItem from "./TreeItem.vue";
import { RecycleScroller } from "vue-virtual-scroller";
import "vue-virtual-scroller/dist/vue-virtual-scroller.css";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";

const { t } = useI18n();
const store = useConnectionStore();
const queryStore = useQueryStore();
const settingsStore = useSettingsStore();
const searchQuery = ref("");
const deferredSearchQuery = ref("");
const searchInputRef = ref<HTMLInputElement>();
const treeScrollerRef = ref<InstanceType<typeof RecycleScroller> | null>(null);
const plainTreeScrollerRef = ref<HTMLElement | null>(null);
type SearchScope = "connection" | "database" | "schema" | "table" | "view";
const selectedSearchScopes = ref<SearchScope[]>([]);
const searchCollapsedIds = ref<Set<string>>(new Set());
let searchTimer: number | undefined;

watch(
  searchQuery,
  (value) => {
    const normalized = value.trim().toLowerCase();
    window.clearTimeout(searchTimer);

    if (!normalized) {
      deferredSearchQuery.value = "";
      return;
    }

    searchTimer = window.setTimeout(() => {
      deferredSearchQuery.value = normalized;
    }, 120);
  },
  { flush: "sync" },
);

const isSearching = computed(() => !!deferredSearchQuery.value);
const isFiltering = computed(() => !!searchQuery.value.trim() || hasSearchScopeFilter.value);

const SEARCH_SCOPE_TO_NODE_TYPES: Record<SearchScope, TreeNodeType[]> = {
  connection: ["connection"],
  database: ["database", "redis-db", "mongo-db"],
  schema: ["schema"],
  table: ["table", "mongo-collection"],
  view: ["view"],
};

const searchScopeOptions = computed(() => {
  return [
    { scope: "connection", label: t("sidebar.searchScopeConnection") },
    { scope: "database", label: t("sidebar.searchScopeDatabase") },
    { scope: "schema", label: t("sidebar.searchScopeSchema") },
    { scope: "table", label: t("sidebar.searchScopeTable") },
    { scope: "view", label: t("sidebar.searchScopeView") },
  ] as const satisfies ReadonlyArray<{ scope: SearchScope; label: string }>;
});

const hasSearchScopeFilter = computed(() => selectedSearchScopes.value.length > 0);
const searchableNodeTypes = computed<Set<TreeNodeType> | undefined>(() => {
  if (!hasSearchScopeFilter.value) return undefined;
  const types = new Set<TreeNodeType>();
  for (const scope of selectedSearchScopes.value) {
    for (const nodeType of SEARCH_SCOPE_TO_NODE_TYPES[scope]) {
      types.add(nodeType);
    }
  }
  return types;
});

function isSearchScopeSelected(scope: SearchScope) {
  return selectedSearchScopes.value.includes(scope);
}

function toggleSearchScope(scope: SearchScope) {
  const idx = selectedSearchScopes.value.indexOf(scope);
  if (idx >= 0) {
    selectedSearchScopes.value.splice(idx, 1);
  } else {
    selectedSearchScopes.value.push(scope);
  }
}

function clearSearchScopeFilter() {
  selectedSearchScopes.value = [];
}

const filteredNodes = computed(() => {
  let nodes = store.treeNodes;

  const q = deferredSearchQuery.value;
  if (q) {
    nodes = filterSidebarTree(nodes, q, searchCollapsedIds.value, searchableNodeTypes.value);
  }

  return nodes;
});

const flatNodes = computed<FlatTreeNode[]>(() => flattenTree(filteredNodes.value));
const useVirtualTree = computed(() => shouldVirtualizeFlatTree(flatNodes.value.length));
const activeTab = computed(() => queryStore.tabs.find((tab) => tab.id === queryStore.activeTabId));

const pendingRenameGroupId = ref<string | null>(null);

function createNewGroup() {
  const groupId = store.createConnectionGroup(t("connectionGroup.newGroupDefault"));
  pendingRenameGroupId.value = groupId;
}

function onSearchToggle(node: TreeNode) {
  if (!isSearching.value || !node.children) return;
  const next = new Set(searchCollapsedIds.value);
  if (node.isExpanded) next.add(node.id);
  else next.delete(node.id);
  searchCollapsedIds.value = next;
}

async function onNodeToggled(node: TreeNode, wasExpanded: boolean) {
  if (wasExpanded || !node.isExpanded) return;
  if (!shouldAutoScrollExpandedTreeNode(node.type)) return;

  await nextTick();

  const expandedIndex = flatNodes.value.findIndex((item) => item.id === node.id);
  const insertedRowCount = flattenTree([node]).length - 1;
  const scroller = treeScrollerRef.value?.$el as HTMLElement | undefined;
  if (!scroller || expandedIndex < 0 || insertedRowCount <= 0) return;

  const nextScrollTop = scrollTopForExpandedTreeNode({
    expandedIndex,
    insertedRowCount,
    currentScrollTop: scroller.scrollTop,
    viewportHeight: scroller.clientHeight,
  });

  if (nextScrollTop !== scroller.scrollTop) {
    scroller.scrollTop = nextScrollTop;
  }
}

function currentTreeScroller(): HTMLElement | null {
  return (
    ((useVirtualTree.value ? treeScrollerRef.value?.$el : plainTreeScrollerRef.value) as HTMLElement | undefined) ??
    null
  );
}

async function selectActiveTabSidebarNode() {
  if (!settingsStore.editorSettings.autoSelectActiveSidebarNode) return;
  const match = findSidebarNodeForActiveTab(activeTab.value, flatNodes.value);
  if (!match) return;

  store.selectedTreeNodeId = match.id;
  await nextTick();

  const index = flatNodes.value.findIndex((item) => item.id === match.id);
  const scroller = currentTreeScroller();
  if (!scroller || index < 0) return;

  const nextScrollTop = scrollTopForSidebarNode({
    index,
    currentScrollTop: scroller.scrollTop,
    viewportHeight: scroller.clientHeight,
  });
  if (nextScrollTop !== scroller.scrollTop) {
    scroller.scrollTop = nextScrollTop;
  }
}

watch(
  [activeTab, flatNodes, () => settingsStore.editorSettings.autoSelectActiveSidebarNode],
  () => {
    void selectActiveTabSidebarNode();
  },
  { flush: "post" },
);

function focusSearch(): boolean {
  const input = searchInputRef.value;
  if (!input) return false;
  input.focus();
  input.select();
  return true;
}

function onSearchKeydown(event: KeyboardEvent) {
  if (!isCancelSearchShortcut(event)) return;
  event.preventDefault();
  searchQuery.value = "";
}

defineExpose({ focusSearch });
</script>

<template>
  <div class="h-full min-h-0 flex flex-col text-sm select-none">
    <div class="sticky top-0 z-10 bg-background px-2 py-1">
      <div class="relative flex items-center gap-1">
        <div class="relative flex-1">
          <Search class="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
            class="w-full h-6 pl-7 pr-6 text-xs rounded border border-border bg-background focus:outline-none focus:ring-1 focus:ring-ring"
            :placeholder="t('grid.search')"
            @keydown="onSearchKeydown"
          />
          <button
            v-if="searchQuery"
            class="absolute right-1.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            @click="searchQuery = ''"
          >
            <X class="h-3 w-3" />
          </button>
        </div>
        <button
          class="shrink-0 h-6 w-6 flex items-center justify-center rounded border border-border text-muted-foreground hover:bg-accent hover:text-foreground"
          :title="t('connectionGroup.createGroup')"
          @click="createNewGroup"
        >
          <FolderPlus class="h-3.5 w-3.5" />
        </button>
        <DropdownMenu v-if="searchScopeOptions.length > 0">
          <DropdownMenuTrigger as-child>
            <button
              class="shrink-0 h-6 w-6 flex items-center justify-center rounded border border-border hover:bg-accent"
              :class="hasSearchScopeFilter ? 'text-primary bg-primary/10 border-primary/30' : 'text-muted-foreground'"
              :title="t('sidebar.filterByType')"
            >
              <ListFilter class="h-3.5 w-3.5" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" class="w-48">
            <DropdownMenuLabel class="text-xs">{{ t("sidebar.filterByType") }}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              v-for="item in searchScopeOptions"
              :key="item.scope"
              class="gap-2"
              :class="isSearchScopeSelected(item.scope) ? 'bg-primary/10 text-primary' : ''"
              @select.prevent="toggleSearchScope(item.scope)"
            >
              <Check v-if="isSearchScopeSelected(item.scope)" class="h-3.5 w-3.5 shrink-0 text-primary" />
              <span v-else class="h-3.5 w-3.5 shrink-0" />
              <span class="flex-1 truncate">{{ item.label }}</span>
            </DropdownMenuItem>
            <template v-if="hasSearchScopeFilter">
              <DropdownMenuSeparator />
              <DropdownMenuItem @select.prevent="clearSearchScopeFilter">
                <span class="text-xs text-muted-foreground">{{ t("sidebar.clearFilter") }}</span>
              </DropdownMenuItem>
            </template>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </div>
    <RecycleScroller
      v-if="flatNodes.length > 0 && useVirtualTree"
      ref="treeScrollerRef"
      class="sidebar-tree connection-tree-scroller min-h-0 flex-1 overflow-y-auto overflow-x-auto"
      :items="flatNodes"
      :item-size="SIDEBAR_TREE_ROW_HEIGHT"
      :buffer="SIDEBAR_TREE_SCROLL_BUFFER"
      :prerender="SIDEBAR_TREE_PRERENDER_COUNT"
      :skip-hover="true"
      key-field="id"
      type-field="type"
      flow-mode
    >
      <template #default="{ item }">
        <TreeItem
          :node="item.node"
          :depth="item.depth"
          :drag-disabled="isFiltering"
          :pending-rename="pendingRenameGroupId === item.node.id"
          @node-toggled="onNodeToggled"
          @search-toggle="onSearchToggle"
          @rename-started="pendingRenameGroupId = null"
        />
      </template>
    </RecycleScroller>
    <div
      v-else-if="flatNodes.length > 0"
      ref="plainTreeScrollerRef"
      class="sidebar-tree min-h-0 flex-1 overflow-y-auto overflow-x-auto"
    >
      <TreeItem
        v-for="item in flatNodes"
        :key="item.id"
        :node="item.node"
        :depth="item.depth"
        :drag-disabled="isFiltering"
        :pending-rename="pendingRenameGroupId === item.node.id"
        @node-toggled="onNodeToggled"
        @search-toggle="onSearchToggle"
        @rename-started="pendingRenameGroupId = null"
      />
    </div>
    <div v-if="store.treeNodes.length === 0" class="px-3 py-8 text-center text-muted-foreground text-xs">
      {{ t("sidebar.noConnections") }}
    </div>
  </div>
</template>

<style scoped>
.connection-tree-scroller {
  will-change: scroll-position;
  contain: content;
}

.connection-tree-scroller :deep(.vue-recycle-scroller__item-view) {
  contain: layout style paint;
}
</style>
