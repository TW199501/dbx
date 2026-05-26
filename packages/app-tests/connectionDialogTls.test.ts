import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(
  new URL("../../apps/desktop/src/components/connection/ConnectionDialog.vue", import.meta.url),
  "utf8",
);

test("connection dialog exposes generic TLS controls for supported database types", () => {
  assert.match(source, /const tlsCapableDatabaseTypes = new Set<DatabaseType>/);
  assert.match(source, /"mysql"/);
  assert.match(source, /"postgres"/);
  assert.match(source, /const supportsTlsToggle = computed/);
  assert.match(source, /<TabsTrigger v-if="supportsTlsToggle" value="tls">/);
  assert.match(source, /<TabsContent v-if="supportsTlsToggle" value="tls"/);
});

test("connection dialog exposes CA certificate path for native MySQL TLS", () => {
  assert.match(source, /const supportsCaCertificatePath = computed/);
  assert.match(source, /form\.value\.db_type === "mysql"/);
  assert.match(source, /v-if="supportsCaCertificatePath"/);
  assert.match(source, /v-model="form\.ca_cert_path"/);
});
