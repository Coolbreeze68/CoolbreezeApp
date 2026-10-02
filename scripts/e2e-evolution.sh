#!/usr/bin/env bash
# Scénario de bout en bout : création du CRM, saisie de données, évolution du
# schéma (ajout et renommage de colonnes), migration, puis vérification que les
# données et le code personnalisé sont conservés.
#
# Base : DATABASE_URL (SQLite temporaire par défaut). Avec PostgreSQL ou MySQL,
# utilisez une base vide dédiée.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
export DATABASE_URL="${DATABASE_URL:-sqlite://$WORK/crm.db?mode=rwc}"
export FORGE_ADDR=127.0.0.1:18080
export RUST_LOG=warn
# Les migrations sont appliquées explicitement par `forge migrate`.
export FORGE_AUTO_MIGRATE=false
API="http://$FORGE_ADDR/api"
FORGE="$CARGO_TARGET_DIR/debug/forge"
PID=

step() { printf '\n==> %s\n' "$*"; }
fail() { echo "ÉCHEC : $*" >&2; exit 1; }
cleanup() { [ -n "$PID" ] && kill "$PID" 2>/dev/null || true; }
trap cleanup EXIT

start_server() {
    cargo build -q --manifest-path backend/Cargo.toml
    "$CARGO_TARGET_DIR/debug/mini_crm" &
    PID=$!
    for _ in $(seq 50); do
        curl -sf "$API/parameters" >/dev/null && return
        sleep 0.2
    done
    fail "le serveur ne répond pas"
}

stop_server() {
    kill "$PID"
    wait "$PID" 2>/dev/null || true
    PID=
}

# Applique une transformation Python au schéma `forge.json`.
edit_schema() {
    python3 - "$1" <<'PY'
import json, sys
schema = json.load(open("forge.json"))
exec(sys.argv[1])
json.dump(schema, open("forge.json", "w"), indent=2, ensure_ascii=False)
PY
}

step "Création du projet"
cargo build -q --manifest-path "$ROOT/Cargo.toml" -p forge-cli
"$FORGE" new "$WORK/crm" --schema "$ROOT/examples/crm/forge.json" >/dev/null
cd "$WORK/crm"
echo "// code personnalisé" >> backend/src/custom/routes.rs
cp backend/src/custom/routes.rs "$WORK/routes.rs"
"$FORGE" migrate up --dir .

step "Saisie de données"
start_server
curl -sf -X POST "$API/entreprise" -H 'content-type: application/json' \
    -d '{"nom": "Acme", "ville": "Lyon", "chiffre_affaires": "1500"}' >/dev/null
stop_server

step 'Évolution du schéma : nouvelle colonne effectif, ville renommée en commune'
edit_schema '
columns = schema["tables"][0]["columns"]
columns.append({"name": "effectif", "type": "integer", "default": 10})
ville = next(c for c in columns if c["name"] == "ville")
ville["name"], ville["renamed_from"] = "commune", "ville"
'
"$FORGE" generate --dir . | tee "$WORK/generate.log"
grep -q "Migration m0002_entreprise" "$WORK/generate.log" || fail "migration non générée"

step "Migration"
"$FORGE" migrate up --dir .

check_record() {
    start_server
    record=$(curl -sf "$API/entreprise/1")
    stop_server
    echo "$record"
    python3 - "$record" <<'PY'
import json, sys
record = json.loads(sys.argv[1])
assert record["nom"] == "Acme", record
assert record["commune"] == "Lyon", "renommage sans perte"
assert record["effectif"] == 10, "valeur par défaut appliquée aux lignes existantes"
assert "ville" not in record, record
PY
}

step "Vérification"
check_record

step "Retour arrière puis nouvelle application de la migration"
"$FORGE" migrate down --dir .
"$FORGE" migrate up --dir .
check_record
cmp -s backend/src/custom/routes.rs "$WORK/routes.rs" || fail "code personnalisé modifié"
"$FORGE" generate --dir . | grep -q "Aucun changement" || fail "génération non idempotente"

step "Succès"
