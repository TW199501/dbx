<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useConnectionStore } from "@/stores/connectionStore";
import { useToast } from "@/composables/useToast";
import { isSchemaAware } from "@/lib/databaseCapabilities";
import { copyToClipboard } from "@/lib/clipboard";
import * as api from "@/lib/api";
import DatabaseIcon from "@/components/icons/DatabaseIcon.vue";
import { ArrowLeftRight, CheckSquare, Copy, GitCompareArrows, Loader2, Play, Square } from "lucide-vue-next";

interface CompareColumn {
  name: string;
  is_primary_key?: boolean;
}

interface DataCompareTableTask {
  sourceTable: string;
  targetTable: string;
}

type DataCompareTableStatus = "different" | "same" | "error";

interface DataCompareTableResult {
  sourceTable: string;
  targetTable: string;
  keyColumns: string[];
  status: DataCompareTableStatus;
  added: number;
  removed: number;
  modified: number;
  sourceRowCount: number;
  targetRowCount: number;
  sourceTruncated: boolean;
  targetTruncated: boolean;
  syncStatements: string[];
  syncSql: string;
  error?: string;
}

const { t } = useI18n();
const { toast } = useToast();
const store = useConnectionStore();
const open = defineModel<boolean>("open", { default: false });

const props = defineProps<{
  prefillConnectionId?: string;
  prefillDatabase?: string;
  prefillSchema?: string;
  prefillTable?: string;
}>();

const sourceConnectionId = ref("");
const sourceDatabase = ref("");
const sourceSchema = ref("");
const sourceTable = ref("");
const sourceDatabases = ref<string[]>([]);
const sourceSchemas = ref<string[]>([]);
const sourceTables = ref<string[]>([]);
const sourceTableSearch = ref("");
const selectedSourceTables = ref<Set<string>>(new Set());

const targetConnectionId = ref("");
const targetDatabase = ref("");
const targetSchema = ref("");
const targetTable = ref("");
const targetDatabases = ref<string[]>([]);
const targetSchemas = ref<string[]>([]);
const targetTables = ref<string[]>([]);

const keyColumnsText = ref("");
const rowLimit = ref("1000");
const syncSql = ref("");
const syncStatements = ref<string[]>([]);
const batchResults = ref<DataCompareTableResult[]>([]);
const comparing = ref(false);
const compareProgressCurrent = ref(0);
const compareProgressTotal = ref(0);
const compareProgressTable = ref("");
const executing = ref(false);
const executedCount = ref(0);
const executeTotal = ref(0);
const syncErrors = ref<{ sql: string; error: string }[]>([]);
const rowLimitOptions = [1000, 5000, 10000, 50000];

const sqlConnections = computed(() =>
  store.connections.filter((connection) => !["redis", "mongodb", "elasticsearch"].includes(connection.db_type)),
);
const selectedSourceTableNames = computed(() =>
  sourceTables.value.filter((table) => selectedSourceTables.value.has(table)),
);
const isBatchCompare = computed(() => selectedSourceTableNames.value.length > 1);
const filteredSourceTables = computed(() => {
  const query = sourceTableSearch.value.trim().toLowerCase();
  if (!query) return sourceTables.value;
  return sourceTables.value.filter((table) => table.toLowerCase().includes(query));
});
const allFilteredTablesSelected = computed(
  () =>
    filteredSourceTables.value.length > 0 &&
    filteredSourceTables.value.every((table) => selectedSourceTables.value.has(table)),
);
const compareTasksPreview = computed(() =>
  selectedSourceTableNames.value.map((table) => {
    const target = isBatchCompare.value ? table : targetTable.value || table;
    const matched = !!target && targetTables.value.includes(target);
    return {
      sourceTable: table,
      targetTable: target,
      matched,
    };
  }),
);
const matchedTaskCount = computed(() => compareTasksPreview.value.filter((task) => task.matched).length);
const missingTargetTables = computed(() =>
  compareTasksPreview.value.filter((task) => !task.matched).map((task) => task.targetTable || task.sourceTable),
);
const canCompare = computed(
  () =>
    sourceConnectionId.value &&
    sourceDatabase.value &&
    sourceSchema.value &&
    selectedSourceTableNames.value.length > 0 &&
    targetConnectionId.value &&
    targetDatabase.value &&
    targetSchema.value &&
    (!isBatchCompare.value ? !!targetTable.value : true),
);
const keyColumns = computed(() =>
  keyColumnsText.value
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean),
);
const rowLimitNumber = computed(() => Number(rowLimit.value) || 1000);

const sameTableCount = computed(() => batchResults.value.filter((item) => item.status === "same").length);
const differentTableCount = computed(() => batchResults.value.filter((item) => item.status === "different").length);
const failedTableCount = computed(() => batchResults.value.filter((item) => item.status === "error").length);
const totalAdded = computed(() => batchResults.value.reduce((sum, item) => sum + item.added, 0));
const totalRemoved = computed(() => batchResults.value.reduce((sum, item) => sum + item.removed, 0));
const totalModified = computed(() => batchResults.value.reduce((sum, item) => sum + item.modified, 0));
const hasAnyTruncated = computed(() => batchResults.value.some((item) => item.sourceTruncated || item.targetTruncated));
const hasResults = computed(() => batchResults.value.length > 0);
const summary = computed(() => {
  if (!hasResults.value) return "";
  if (batchResults.value.length === 1 && batchResults.value[0]?.status !== "error") {
    const item = batchResults.value[0];
    return t("dataCompare.summary", {
      added: item.added,
      removed: item.removed,
      modified: item.modified,
    });
  }
  return t("dataCompare.batchSummary", {
    tables: batchResults.value.length,
    different: differentTableCount.value,
    same: sameTableCount.value,
    failed: failedTableCount.value,
    added: totalAdded.value,
    removed: totalRemoved.value,
    modified: totalModified.value,
  });
});
const compareProgressLabel = computed(() => {
  if (!comparing.value || compareProgressTotal.value === 0) return "";
  return t("dataCompare.comparingTable", {
    current: compareProgressCurrent.value,
    total: compareProgressTotal.value,
    table: compareProgressTable.value,
  });
});

function connectionIconType(connectionId: string) {
  const config = store.getConfig(connectionId);
  return config?.driver_profile || config?.db_type || "mysql";
}

function resetSelectedSourceTables(nextTables: Iterable<string>) {
  selectedSourceTables.value = new Set(nextTables);
}

function toggleSourceTable(table: string) {
  const next = new Set(selectedSourceTables.value);
  if (next.has(table)) next.delete(table);
  else next.add(table);
  resetSelectedSourceTables(next);
}

function toggleSelectAllSourceTables() {
  const next = new Set(selectedSourceTables.value);
  if (allFilteredTablesSelected.value) {
    filteredSourceTables.value.forEach((table) => next.delete(table));
  } else {
    filteredSourceTables.value.forEach((table) => next.add(table));
  }
  resetSelectedSourceTables(next);
}

function buildCompareTasks(): DataCompareTableTask[] {
  if (!selectedSourceTableNames.value.length) return [];
  if (!isBatchCompare.value) {
    const table = selectedSourceTableNames.value[0];
    return targetTable.value ? [{ sourceTable: table, targetTable: targetTable.value }] : [];
  }
  return selectedSourceTableNames.value.map((table) => ({
    sourceTable: table,
    targetTable: table,
  }));
}

function clearResult() {
  batchResults.value = [];
  syncSql.value = "";
  syncStatements.value = [];
  syncErrors.value = [];
  compareProgressCurrent.value = 0;
  compareProgressTotal.value = 0;
  compareProgressTable.value = "";
}

function swapSourceTarget() {
  const previousSelectedTables = [...selectedSourceTableNames.value];
  const nextSingleTarget = previousSelectedTables.length === 1 ? (previousSelectedTables[0] ?? "") : "";
  const nextSourceSelection =
    previousSelectedTables.length <= 1 ? [targetTable.value].filter(Boolean) : previousSelectedTables;

  const tmpConnId = sourceConnectionId.value;
  const tmpDb = sourceDatabase.value;
  const tmpDbs = sourceDatabases.value;
  const tmpSchema = sourceSchema.value;
  const tmpSchemas = sourceSchemas.value;
  const tmpTables = sourceTables.value;

  sourceConnectionId.value = targetConnectionId.value;
  sourceDatabase.value = targetDatabase.value;
  sourceDatabases.value = targetDatabases.value;
  sourceSchema.value = targetSchema.value;
  sourceSchemas.value = targetSchemas.value;
  sourceTables.value = targetTables.value;
  resetSelectedSourceTables(nextSourceSelection.filter((table) => targetTables.value.includes(table)));

  targetConnectionId.value = tmpConnId;
  targetDatabase.value = tmpDb;
  targetDatabases.value = tmpDbs;
  targetSchema.value = tmpSchema;
  targetSchemas.value = tmpSchemas;
  targetTables.value = tmpTables;
  targetTable.value = nextSingleTarget;

  sourceTable.value = selectedSourceTableNames.value.length === 1 ? selectedSourceTableNames.value[0] : "";
  clearResult();
}

async function resolveSchema(connectionId: string, database: string, preferredSchema = ""): Promise<string> {
  const config = store.getConfig(connectionId);
  if (isSchemaAware(config?.db_type)) {
    const schemas = await api.listSchemas(connectionId, database);
    if (preferredSchema && schemas.includes(preferredSchema)) return preferredSchema;
    return schemas.includes("public") ? "public" : (schemas[0] ?? "");
  }
  return database;
}

async function loadSchemas(side: "source" | "target", preferredSchema = "") {
  const connectionId = side === "source" ? sourceConnectionId.value : targetConnectionId.value;
  const database = side === "source" ? sourceDatabase.value : targetDatabase.value;
  if (!connectionId || !database) return;
  const config = store.getConfig(connectionId);
  if (!isSchemaAware(config?.db_type)) {
    if (side === "source") {
      sourceSchemas.value = [];
      sourceSchema.value = database;
    } else {
      targetSchemas.value = [];
      targetSchema.value = database;
    }
    await loadTables(side);
    return;
  }

  const schemas = await api.listSchemas(connectionId, database);
  const schema =
    preferredSchema && schemas.includes(preferredSchema)
      ? preferredSchema
      : schemas.includes("public")
        ? "public"
        : (schemas[0] ?? "");
  if (side === "source") {
    sourceSchemas.value = schemas;
    sourceSchema.value = schema;
  } else {
    targetSchemas.value = schemas;
    targetSchema.value = schema;
  }
}

async function loadDatabases(connectionId: string, side: "source" | "target") {
  if (!connectionId) return;
  await store.ensureConnected(connectionId);
  const names = (await api.listDatabases(connectionId)).map((database) => database.name);
  if (side === "source") {
    sourceDatabases.value = names;
    sourceDatabase.value = names.length === 1 ? names[0] : "";
    sourceSchemas.value = [];
    sourceSchema.value = "";
    sourceTables.value = [];
    sourceTable.value = "";
    resetSelectedSourceTables([]);
  } else {
    targetDatabases.value = names;
    targetDatabase.value = names.length === 1 ? names[0] : "";
    targetSchemas.value = [];
    targetSchema.value = "";
    targetTables.value = [];
    targetTable.value = "";
  }
}

async function loadTables(side: "source" | "target") {
  const connectionId = side === "source" ? sourceConnectionId.value : targetConnectionId.value;
  const database = side === "source" ? sourceDatabase.value : targetDatabase.value;
  if (!connectionId || !database) return;
  const schema =
    side === "source"
      ? sourceSchema.value || (await resolveSchema(connectionId, database, props.prefillSchema))
      : targetSchema.value || (await resolveSchema(connectionId, database));
  const tables = (await api.listTables(connectionId, database, schema))
    .filter((table) => table.table_type !== "VIEW")
    .map((table) => table.name);

  if (side === "source") {
    const preferredSelection =
      props.prefillTable && tables.includes(props.prefillTable)
        ? [props.prefillTable]
        : [...selectedSourceTables.value].filter((table) => tables.includes(table));
    sourceSchema.value = schema;
    sourceTables.value = tables;
    resetSelectedSourceTables(preferredSelection);
    sourceTable.value = preferredSelection.length === 1 ? preferredSelection[0] : "";
  } else {
    targetSchema.value = schema;
    targetTables.value = tables;
    const singleSourceTable = selectedSourceTableNames.value.length === 1 ? selectedSourceTableNames.value[0] : "";
    const preferred =
      targetTable.value && tables.includes(targetTable.value)
        ? targetTable.value
        : singleSourceTable && tables.includes(singleSourceTable)
          ? singleSourceTable
          : "";
    targetTable.value = preferred;
  }
}

async function loadColumnsWithCache(
  cache: Map<string, CompareColumn[]>,
  connectionId: string,
  database: string,
  schema: string,
  table: string,
): Promise<CompareColumn[]> {
  const key = `${connectionId}:${database}:${schema}:${table}`;
  const cached = cache.get(key);
  if (cached) return cached;
  const columns = (await api.getColumns(connectionId, database, schema, table)) as CompareColumn[];
  cache.set(key, columns);
  return columns;
}

async function inferKeyColumnsForTable(
  table: string,
  sourceColumnCache?: Map<string, CompareColumn[]>,
): Promise<string[]> {
  if (!sourceConnectionId.value || !sourceDatabase.value || !sourceSchema.value || !table) return [];
  const columns = sourceColumnCache
    ? await loadColumnsWithCache(
        sourceColumnCache,
        sourceConnectionId.value,
        sourceDatabase.value,
        sourceSchema.value,
        table,
      )
    : (((await api.getColumns(
        sourceConnectionId.value,
        sourceDatabase.value,
        sourceSchema.value,
        table,
      )) as CompareColumn[]) ?? []);
  const primaryKeys = columns.filter((column) => column.is_primary_key).map((column) => column.name);
  if (primaryKeys.length > 0) return primaryKeys;
  return columns.slice(0, 1).map((column) => column.name);
}

async function inferKeyColumns() {
  const table = selectedSourceTableNames.value.length === 1 ? selectedSourceTableNames.value[0] : "";
  if (!table) return;
  const inferred = await inferKeyColumnsForTable(table);
  keyColumnsText.value = inferred.join(", ");
}

function resultStatusLabel(status: DataCompareTableStatus): string {
  if (status === "different") return t("dataCompare.statusDifferent");
  if (status === "same") return t("dataCompare.statusSame");
  return t("dataCompare.statusError");
}

function resultStatusClass(status: DataCompareTableStatus): string {
  if (status === "different") return "bg-amber-500/15 text-amber-700";
  if (status === "same") return "bg-emerald-500/15 text-emerald-700";
  return "bg-destructive/15 text-destructive";
}

async function startCompare() {
  if (!canCompare.value || comparing.value) return;
  const tasks = buildCompareTasks();
  if (tasks.length === 0) {
    toast(t("dataCompare.noComparableTables"), 5000);
    return;
  }

  comparing.value = true;
  clearResult();
  compareProgressTotal.value = tasks.length;

  const sourceColumnCache = new Map<string, CompareColumn[]>();
  const targetColumnCache = new Map<string, CompareColumn[]>();
  const results: DataCompareTableResult[] = [];
  const mergedStatements: string[] = [];
  const mergedSqlParts: string[] = [];

  try {
    await Promise.all([
      store.ensureConnected(sourceConnectionId.value),
      store.ensureConnected(targetConnectionId.value),
    ]);

    for (const [index, task] of tasks.entries()) {
      compareProgressCurrent.value = index + 1;
      compareProgressTable.value = task.sourceTable;

      try {
        if (!targetTables.value.includes(task.targetTable)) {
          throw new Error(t("dataCompare.targetTableMissing", { table: task.targetTable }));
        }

        const resolvedKeys =
          keyColumns.value.length > 0
            ? keyColumns.value
            : await inferKeyColumnsForTable(task.sourceTable, sourceColumnCache);
        if (resolvedKeys.length === 0) {
          throw new Error(t("dataCompare.noKeyColumns"));
        }

        const sourceColumns = await loadColumnsWithCache(
          sourceColumnCache,
          sourceConnectionId.value,
          sourceDatabase.value,
          sourceSchema.value,
          task.sourceTable,
        );
        const targetColumns = await loadColumnsWithCache(
          targetColumnCache,
          targetConnectionId.value,
          targetDatabase.value,
          targetSchema.value,
          task.targetTable,
        );
        const columns = sourceColumns
          .map((column) => column.name)
          .filter((column) => targetColumns.some((target) => target.name === column));
        const missingKeys = resolvedKeys.filter((column) => !columns.includes(column));
        if (missingKeys.length > 0) {
          throw new Error(t("dataCompare.missingKeyColumns", { columns: missingKeys.join(", ") }));
        }
        if (columns.length === 0) {
          throw new Error(t("dataCompare.noCommonColumns"));
        }

        const preparation = await api.prepareDataCompareFromTables({
          sourceConnectionId: sourceConnectionId.value,
          sourceDatabase: sourceDatabase.value,
          sourceSchema: sourceSchema.value,
          sourceTable: task.sourceTable,
          targetConnectionId: targetConnectionId.value,
          targetDatabase: targetDatabase.value,
          targetSchema: targetSchema.value,
          targetTable: task.targetTable,
          columns,
          keyColumns: resolvedKeys,
          rowLimit: rowLimitNumber.value,
        });

        const added = preparation.result.added.length;
        const removed = preparation.result.removed.length;
        const modified = preparation.result.modified.length;
        const status: DataCompareTableStatus = added || removed || modified ? "different" : "same";

        results.push({
          sourceTable: task.sourceTable,
          targetTable: task.targetTable,
          keyColumns: resolvedKeys,
          status,
          added,
          removed,
          modified,
          sourceRowCount: preparation.sourceRowCount,
          targetRowCount: preparation.targetRowCount,
          sourceTruncated: preparation.sourceTruncated,
          targetTruncated: preparation.targetTruncated,
          syncStatements: preparation.syncStatements,
          syncSql: preparation.syncSql,
        });

        if (preparation.syncStatements.length > 0) {
          mergedStatements.push(...preparation.syncStatements);
        }
        if (preparation.syncSql.trim()) {
          mergedSqlParts.push(preparation.syncSql.trim());
        }
      } catch (e: any) {
        results.push({
          sourceTable: task.sourceTable,
          targetTable: task.targetTable,
          keyColumns: keyColumns.value,
          status: "error",
          added: 0,
          removed: 0,
          modified: 0,
          sourceRowCount: 0,
          targetRowCount: 0,
          sourceTruncated: false,
          targetTruncated: false,
          syncStatements: [],
          syncSql: "",
          error: e?.message || String(e),
        });
      }
    }

    batchResults.value = results;
    syncStatements.value = mergedStatements;
    syncSql.value = mergedSqlParts.join("\n\n");
  } catch (e: any) {
    toast(e?.message || String(e), 5000);
  } finally {
    comparing.value = false;
    compareProgressCurrent.value = 0;
    compareProgressTotal.value = 0;
    compareProgressTable.value = "";
  }
}

async function copySql() {
  try {
    await copyToClipboard(syncSql.value);
    toast(t("grid.copied"));
  } catch (e: any) {
    toast(t("grid.copyFailed", { message: e?.message || String(e) }), 5000);
  }
}

async function executeSql() {
  if (!syncSql.value.trim() || syncStatements.value.length === 0 || executing.value) return;
  executing.value = true;
  syncErrors.value = [];
  executeTotal.value = syncStatements.value.length;
  executedCount.value = 0;
  try {
    await store.ensureConnected(targetConnectionId.value);
    for (const stmt of syncStatements.value) {
      try {
        await api.executeQuery(targetConnectionId.value, targetDatabase.value, stmt, targetSchema.value);
      } catch (e: any) {
        syncErrors.value.push({ sql: stmt, error: e?.message || String(e) });
      }
      executedCount.value++;
    }
    const failed = syncErrors.value.length;
    if (failed === 0) {
      toast(t("dataCompare.syncSuccess"), 2000);
    } else {
      toast(t("diff.syncSummary", { success: syncStatements.value.length - failed, failed }), 5000);
    }
  } catch (e: any) {
    toast(e?.message || String(e), 5000);
  } finally {
    executing.value = false;
  }
}

watch(sourceConnectionId, (id) => {
  clearResult();
  sourceDatabase.value = "";
  sourceSchema.value = "";
  sourceSchemas.value = [];
  sourceTables.value = [];
  sourceTable.value = "";
  sourceTableSearch.value = "";
  resetSelectedSourceTables([]);
  loadDatabases(id, "source").catch((e) => toast(String(e), 5000));
});
watch(targetConnectionId, (id) => {
  clearResult();
  targetDatabase.value = "";
  targetSchema.value = "";
  targetSchemas.value = [];
  targetTables.value = [];
  targetTable.value = "";
  loadDatabases(id, "target").catch((e) => toast(String(e), 5000));
});
watch(sourceDatabase, () => {
  clearResult();
  sourceSchema.value = "";
  sourceSchemas.value = [];
  sourceTables.value = [];
  sourceTable.value = "";
  sourceTableSearch.value = "";
  resetSelectedSourceTables([]);
  loadSchemas("source", props.prefillSchema).catch((e) => toast(String(e), 5000));
});
watch(targetDatabase, () => {
  clearResult();
  targetSchema.value = "";
  targetSchemas.value = [];
  targetTables.value = [];
  targetTable.value = "";
  loadSchemas("target").catch((e) => toast(String(e), 5000));
});
watch(sourceSchema, () => {
  clearResult();
  sourceTables.value = [];
  sourceTable.value = "";
  sourceTableSearch.value = "";
  resetSelectedSourceTables([]);
  if (sourceSchema.value) loadTables("source").catch((e) => toast(String(e), 5000));
});
watch(targetSchema, () => {
  clearResult();
  targetTables.value = [];
  targetTable.value = "";
  if (targetSchema.value) loadTables("target").catch((e) => toast(String(e), 5000));
});
watch(selectedSourceTableNames, (tables, previous) => {
  clearResult();
  sourceTable.value = tables.length === 1 ? tables[0] : "";
  if (tables.length !== 1) {
    keyColumnsText.value = "";
    return;
  }
  const table = tables[0];
  if (targetTables.value.includes(table)) {
    targetTable.value = table;
  } else if (previous?.length === 1 && targetTable.value === previous[0]) {
    targetTable.value = "";
  }
  if (table !== previous?.[0]) {
    keyColumnsText.value = "";
  }
  inferKeyColumns().catch(() => {});
});
watch(targetTable, () => clearResult());
watch(
  open,
  async (value) => {
    if (!value) return;
    clearResult();
    if (props.prefillConnectionId) {
      sourceConnectionId.value = props.prefillConnectionId;
      await loadDatabases(props.prefillConnectionId, "source");
      if (props.prefillDatabase) sourceDatabase.value = props.prefillDatabase;
      if (props.prefillDatabase) await loadSchemas("source", props.prefillSchema);
      if (props.prefillTable) {
        await loadTables("source");
        if (sourceTables.value.includes(props.prefillTable)) {
          resetSelectedSourceTables([props.prefillTable]);
          sourceTable.value = props.prefillTable;
        }
      }
    }
  },
  { immediate: true },
);
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-4xl max-h-[85vh] flex flex-col overflow-hidden" @interact-outside.prevent>
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <GitCompareArrows class="w-4 h-4" />
          {{ t("dataCompare.title") }}
        </DialogTitle>
      </DialogHeader>

      <div class="flex-1 min-h-0 overflow-auto space-y-4 py-2">
        <div class="grid grid-cols-[1fr_auto_1fr] gap-4 items-start">
          <div class="space-y-2">
            <Label class="text-xs font-medium">{{ t("diff.source") }}</Label>
            <Select
              :model-value="sourceConnectionId"
              @update:model-value="(v: any) => (sourceConnectionId = String(v))"
            >
              <SelectTrigger class="h-8 text-xs">
                <div class="flex items-center gap-2">
                  <DatabaseIcon
                    v-if="sourceConnectionId"
                    :db-type="connectionIconType(sourceConnectionId)"
                    class="w-3.5 h-3.5"
                  />
                  <SelectValue :placeholder="t('diff.selectConnection')" />
                </div>
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="connection in sqlConnections" :key="connection.id" :value="connection.id">
                  {{ connection.name }}
                </SelectItem>
              </SelectContent>
            </Select>
            <Select :model-value="sourceDatabase" @update:model-value="(v: any) => (sourceDatabase = String(v))">
              <SelectTrigger class="h-8 text-xs"><SelectValue :placeholder="t('diff.selectDatabase')" /></SelectTrigger>
              <SelectContent>
                <SelectItem v-for="database in sourceDatabases" :key="database" :value="database">{{
                  database
                }}</SelectItem>
              </SelectContent>
            </Select>
            <Select
              v-if="sourceSchemas.length"
              :model-value="sourceSchema"
              @update:model-value="(v: any) => (sourceSchema = String(v))"
            >
              <SelectTrigger class="h-8 text-xs"><SelectValue :placeholder="t('diff.selectSchema')" /></SelectTrigger>
              <SelectContent>
                <SelectItem v-for="schema in sourceSchemas" :key="schema" :value="schema">{{ schema }}</SelectItem>
              </SelectContent>
            </Select>

            <div class="space-y-2 rounded-lg border p-2">
              <div class="flex items-center justify-between gap-2">
                <Label class="text-xs font-medium">{{ t("dataCompare.sourceTables") }}</Label>
                <div v-if="sourceTables.length" class="text-[11px] text-muted-foreground">
                  {{
                    t("dataCompare.selectedTables", {
                      selected: selectedSourceTableNames.length,
                      total: sourceTables.length,
                    })
                  }}
                </div>
              </div>

              <Input
                v-if="sourceTables.length > 5"
                v-model="sourceTableSearch"
                class="h-7 text-xs"
                :placeholder="t('dataCompare.searchTables')"
              />

              <div class="flex items-center gap-2">
                <Button
                  v-if="sourceTables.length"
                  variant="outline"
                  size="sm"
                  class="h-7 px-2 text-xs"
                  @click="toggleSelectAllSourceTables"
                >
                  {{
                    allFilteredTablesSelected ? t("dataCompare.deselectAllTables") : t("dataCompare.selectAllTables")
                  }}
                </Button>
              </div>

              <div v-if="!sourceConnectionId || !sourceDatabase" class="text-xs text-muted-foreground py-3 text-center">
                {{ t("dataCompare.selectSourceTables") }}
              </div>
              <div v-else-if="sourceTables.length === 0" class="text-xs text-muted-foreground py-3 text-center">
                {{ t("dataCompare.noTables") }}
              </div>
              <div v-else class="max-h-40 overflow-auto rounded border">
                <button
                  v-for="table in filteredSourceTables"
                  :key="table"
                  type="button"
                  class="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-xs hover:bg-muted/50"
                  @click="toggleSourceTable(table)"
                >
                  <CheckSquare v-if="selectedSourceTables.has(table)" class="w-3.5 h-3.5 text-primary shrink-0" />
                  <Square v-else class="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />
                  <span class="truncate">{{ table }}</span>
                </button>
              </div>
            </div>
          </div>

          <div class="flex items-center pt-6">
            <Button variant="ghost" size="icon" class="h-7 w-7" :title="t('diff.swap')" @click="swapSourceTarget">
              <ArrowLeftRight class="w-3.5 h-3.5" />
            </Button>
          </div>

          <div class="space-y-2">
            <Label class="text-xs font-medium">{{ t("diff.target") }}</Label>
            <Select
              :model-value="targetConnectionId"
              @update:model-value="(v: any) => (targetConnectionId = String(v))"
            >
              <SelectTrigger class="h-8 text-xs">
                <div class="flex items-center gap-2">
                  <DatabaseIcon
                    v-if="targetConnectionId"
                    :db-type="connectionIconType(targetConnectionId)"
                    class="w-3.5 h-3.5"
                  />
                  <SelectValue :placeholder="t('diff.selectConnection')" />
                </div>
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="connection in sqlConnections" :key="connection.id" :value="connection.id">
                  {{ connection.name }}
                </SelectItem>
              </SelectContent>
            </Select>
            <Select :model-value="targetDatabase" @update:model-value="(v: any) => (targetDatabase = String(v))">
              <SelectTrigger class="h-8 text-xs"><SelectValue :placeholder="t('diff.selectDatabase')" /></SelectTrigger>
              <SelectContent>
                <SelectItem v-for="database in targetDatabases" :key="database" :value="database">{{
                  database
                }}</SelectItem>
              </SelectContent>
            </Select>
            <Select
              v-if="targetSchemas.length"
              :model-value="targetSchema"
              @update:model-value="(v: any) => (targetSchema = String(v))"
            >
              <SelectTrigger class="h-8 text-xs"><SelectValue :placeholder="t('diff.selectSchema')" /></SelectTrigger>
              <SelectContent>
                <SelectItem v-for="schema in targetSchemas" :key="schema" :value="schema">{{ schema }}</SelectItem>
              </SelectContent>
            </Select>

            <div v-if="!isBatchCompare" class="space-y-1">
              <Label class="text-xs font-medium">{{ t("dataCompare.targetTable") }}</Label>
              <Select :model-value="targetTable" @update:model-value="(v: any) => (targetTable = String(v))">
                <SelectTrigger class="h-8 text-xs">
                  <SelectValue :placeholder="t('dataCompare.selectTable')" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="table in targetTables" :key="table" :value="table">{{ table }}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div v-else class="space-y-2 rounded-lg border p-3 text-xs">
              <div class="font-medium">{{ t("dataCompare.autoMatchHint") }}</div>
              <div class="text-muted-foreground">
                {{
                  t("dataCompare.matchedTables", { matched: matchedTaskCount, total: selectedSourceTableNames.length })
                }}
              </div>
              <div v-if="missingTargetTables.length" class="text-destructive">
                {{ t("dataCompare.missingTargetTables", { tables: missingTargetTables.join(", ") }) }}
              </div>
              <div v-if="compareTasksPreview.length" class="max-h-36 overflow-auto rounded border bg-muted/20">
                <div
                  v-for="task in compareTasksPreview"
                  :key="`${task.sourceTable}:${task.targetTable}`"
                  class="flex items-center justify-between gap-2 border-b px-2 py-1 last:border-b-0"
                >
                  <span class="truncate font-mono">{{ task.sourceTable }}</span>
                  <span class="text-muted-foreground">→</span>
                  <span class="truncate font-mono" :class="task.matched ? '' : 'text-destructive'">
                    {{ task.targetTable || t("dataCompare.targetTableMissing", { table: task.sourceTable }) }}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div class="space-y-1">
          <Label class="text-xs font-medium">{{ t("dataCompare.keyColumns") }}</Label>
          <Input v-model="keyColumnsText" class="h-8 text-xs" :placeholder="t('dataCompare.keyColumnsPlaceholder')" />
          <div class="text-[11px] text-muted-foreground">
            {{ t("dataCompare.keyColumnsAutoHint") }}
          </div>
        </div>

        <div class="space-y-1">
          <Label class="text-xs font-medium">{{ t("dataCompare.rowLimit") }}</Label>
          <Select v-model="rowLimit">
            <SelectTrigger class="h-8 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="limit in rowLimitOptions" :key="limit" :value="String(limit)">
                {{ t("dataCompare.rowLimitOption", { count: limit }) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div v-if="hasResults" class="space-y-3">
          <div class="rounded-lg border p-3 text-sm">
            {{ summary }}
            <div v-if="hasAnyTruncated" class="mt-1 text-xs text-yellow-600">
              {{ t("dataCompare.truncatedWarning") }}
            </div>
          </div>

          <div class="rounded-lg border overflow-hidden">
            <div class="max-h-64 overflow-auto">
              <table class="w-full text-xs">
                <thead class="bg-muted sticky top-0 z-10">
                  <tr>
                    <th class="px-3 py-2 text-left font-medium">{{ t("diff.table") }}</th>
                    <th class="px-3 py-2 text-left font-medium">{{ t("dataCompare.targetTable") }}</th>
                    <th class="px-3 py-2 text-left font-medium">{{ t("diff.status") }}</th>
                    <th class="px-3 py-2 text-left font-medium">{{ t("diff.details") }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="item in batchResults" :key="`${item.sourceTable}:${item.targetTable}`" class="border-t">
                    <td class="px-3 py-2 align-top font-mono">{{ item.sourceTable }}</td>
                    <td class="px-3 py-2 align-top font-mono text-muted-foreground">{{ item.targetTable }}</td>
                    <td class="px-3 py-2 align-top">
                      <span class="inline-flex rounded px-2 py-0.5 text-[11px]" :class="resultStatusClass(item.status)">
                        {{ resultStatusLabel(item.status) }}
                      </span>
                    </td>
                    <td class="px-3 py-2 align-top text-muted-foreground">
                      <div v-if="item.status === 'error'" class="text-destructive">{{ item.error }}</div>
                      <template v-else>
                        <div>
                          {{
                            t("dataCompare.summary", {
                              added: item.added,
                              removed: item.removed,
                              modified: item.modified,
                            })
                          }}
                        </div>
                        <div class="mt-1">
                          {{
                            t("dataCompare.rowCounts", {
                              source: item.sourceRowCount,
                              target: item.targetRowCount,
                              limit: rowLimitNumber,
                            })
                          }}
                        </div>
                        <div class="mt-1">
                          {{ t("dataCompare.keyColumnsInline", { columns: item.keyColumns.join(", ") }) }}
                        </div>
                      </template>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <div v-if="syncSql.trim()" class="space-y-1">
            <Label class="text-xs font-medium">{{ t("diff.generatedSql") }}</Label>
            <textarea
              v-model="syncSql"
              class="w-full h-48 rounded-lg border bg-muted/20 p-3 font-mono text-xs resize-none focus:outline-none focus:ring-1 focus:ring-ring"
            />
          </div>
          <div v-else-if="differentTableCount === 0 && failedTableCount === 0" class="text-sm text-muted-foreground">
            {{ t("dataCompare.noDifferences") }}
          </div>
        </div>

        <div v-if="syncErrors.length > 0" class="space-y-1">
          <Label class="text-xs font-medium text-destructive">
            {{ t("diff.syncSummary", { success: executeTotal - syncErrors.length, failed: syncErrors.length }) }}
          </Label>
          <div class="max-h-32 overflow-auto border rounded-lg bg-destructive/5 p-2 space-y-1">
            <div v-for="(err, i) in syncErrors" :key="i" class="text-xs font-mono">
              <span class="text-destructive">{{ err.error }}</span>
              <span class="text-muted-foreground ml-1"
                >— {{ err.sql.slice(0, 80) }}{{ err.sql.length > 80 ? "..." : "" }}</span
              >
            </div>
          </div>
        </div>
      </div>

      <DialogFooter v-if="!(hasResults && syncSql.trim())">
        <Button variant="outline" @click="open = false">{{ t("common.close") }}</Button>
        <span v-if="compareProgressLabel" class="text-xs text-muted-foreground self-center">{{
          compareProgressLabel
        }}</span>
        <Button size="sm" :disabled="!canCompare || comparing" @click="startCompare">
          <Loader2 v-if="comparing" class="w-3.5 h-3.5 animate-spin mr-1" />
          <GitCompareArrows v-else class="w-3.5 h-3.5 mr-1" />
          {{ t("dataCompare.compare") }}
        </Button>
      </DialogFooter>

      <DialogFooter v-if="hasResults && syncSql.trim()" class="flex items-center gap-2">
        <span v-if="executing" class="text-xs text-muted-foreground mr-auto">
          {{ t("diff.syncProgress", { current: executedCount, total: executeTotal }) }}
        </span>
        <Button variant="outline" size="sm" @click="copySql">
          <Copy class="w-3 h-3 mr-1" /> {{ t("diff.copySql") }}
        </Button>
        <Button size="sm" :disabled="executing || syncStatements.length === 0" @click="executeSql">
          <Loader2 v-if="executing" class="w-3 h-3 animate-spin mr-1" />
          <Play v-else class="w-3 h-3 mr-1" />
          {{ t("diff.executeSync") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
