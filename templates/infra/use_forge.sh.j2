#!/bin/sh
# Fait pointer le backend et les interfaces vers les sources de forge situées
# dans le dossier donné (image Docker, CI), au lieu du chemin local choisi par
# `forge generate`. Usage : sh scripts/use-forge.sh <sources de forge>
set -eu
forge=$(cd "$1" && pwd)
project=$(cd "$(dirname "$0")/.." && pwd)
if [ -f "$project/backend/Cargo.toml" ]; then
  sed -i -E "s#^(forge-runtime = \{ path = )\"[^\"]*\"#\1\"$forge/crates/forge-runtime\"#" \
    "$project/backend/Cargo.toml"
fi
if [ -f "$project/app/pubspec.yaml" ]; then
  sed -i -E "/^  forge_flutter:/{n;s#^(    path: ).*#\1$forge/packages/forge_flutter#}" \
    "$project/app/pubspec.yaml"
fi
if [ -f "$project/web/package.json" ]; then
  # Chemin relatif : npm l'exige dans le verrouillage des versions, qui désigne
  # aussi le paquet par ce chemin.
  new=$(realpath --relative-to="$project/web" "$forge/packages/forge_web")
  old=$(sed -n -E 's#.*"@forge/web": "file:([^"]*)".*#\1#p' "$project/web/package.json")
  sed -i "s#\"file:$old\"#\"file:$new\"#" "$project/web/package.json"
  if [ -f "$project/web/package-lock.json" ]; then
    sed -i "s#$old\"#$new\"#g" "$project/web/package-lock.json"
  fi
fi
