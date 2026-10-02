#!/bin/sh
# Fait pointer le backend et l'application vers les sources de forge situées
# dans le dossier donné (image Docker, CI), au lieu du chemin local choisi par
# `forge generate`. Usage : sh scripts/use-forge.sh <sources de forge>
set -eu
forge=$(cd "$1" && pwd)
project=$(cd "$(dirname "$0")/.." && pwd)
sed -i -E "s#^(forge-runtime = \{ path = )\"[^\"]*\"#\1\"$forge/crates/forge-runtime\"#" \
  "$project/backend/Cargo.toml"
sed -i -E "/^  forge_flutter:/{n;s#^(    path: ).*#\1$forge/packages/forge_flutter#}" \
  "$project/app/pubspec.yaml"
