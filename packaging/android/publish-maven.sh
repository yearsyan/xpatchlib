#!/usr/bin/env bash
# Assembles a Maven Central bundle from a built AAR — POM, sources jar,
# javadoc jar, every file GPG signed — and optionally uploads it to the
# Central Portal (https://central.sonatype.com). No Gradle involved.
#
#   publish-maven.sh <xpatchlib-<version>.aar> [--upload]
#
# Required environment (GitHub secrets in CI):
#   MAVEN_GPG_KEY          ascii-armored signing key
#   MAVEN_GPG_PASSPHRASE   passphrase for that key
#   CENTRAL_USERNAME       Portal API token username (--upload only)
#   CENTRAL_PASSWORD       Portal API token password (--upload only)
#
# The signing key lives in a scratch GNUPGHOME; ~/.gnupg is never touched.
# NOTE: releases on Maven Central are immutable — a published version can
# never be re-uploaded. Bump the version for any change.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

GROUP=io.github.yearsyan
GROUP_DIR=io/github/yearsyan
ARTIFACT=xpatchlib
PUB_NAME=yearsyan
PUB_EMAIL=yearsyan@hotmail.com
REPO_URL=https://github.com/yearsyan/xpatchlib
DESC='Deterministic binary delta patch replay for app update bundles (XPDL format). Replay-only: patches are produced by the Node toolchain (@lynfe/xpatchlib); no patch generation code ships to devices.'

[[ $# -ge 1 ]] || { echo "usage: $0 <xpatchlib-<version>.aar> [--upload]" >&2; exit 1; }
# Resolve the AAR against the caller's cwd before referencing script-dir paths.
AAR="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
VERSION="${AAR##*/$ARTIFACT-}"; VERSION="${VERSION%.aar}"
[[ -f "$AAR" ]] || { echo "error: AAR not found: $AAR" >&2; exit 1; }

command -v gpg >/dev/null || { echo "error: gpg required" >&2; exit 1; }
command -v javadoc >/dev/null || { echo "error: javadoc (JDK) required" >&2; exit 1; }
[[ -n "${MAVEN_GPG_KEY:-}" && -n "${MAVEN_GPG_PASSPHRASE:-}" ]] || {
  echo "error: MAVEN_GPG_KEY / MAVEN_GPG_PASSPHRASE not set" >&2; exit 1; }

export GNUPGHOME="$(mktemp -d)"
STAGE="$(mktemp -d)"
OUT="$SCRIPT_DIR/build/$ARTIFACT-$VERSION-maven-bundle.zip"
trap 'rm -rf "$GNUPGHOME" "$STAGE"' EXIT

printf '%s' "$MAVEN_GPG_KEY" | gpg --batch --quiet --import
KEYID="$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr/ {print $10; exit}')"
sign() { gpg --batch --yes --quiet --pinentry-mode loopback \
           --passphrase "$MAVEN_GPG_PASSPHRASE" --local-user "$KEYID" \
           --armor --detach-sign "$1"; }

DIR="$STAGE/$GROUP_DIR/$ARTIFACT/$VERSION"
mkdir -p "$DIR"

echo "==> assembling $GROUP:$ARTIFACT:$VERSION"
cp "$AAR" "$DIR/$ARTIFACT-$VERSION.aar"

(cd "$SCRIPT_DIR/src" && zip -qr "$DIR/$ARTIFACT-$VERSION-sources.jar" io)

javadoc -Xdoclint:none -quiet -sourcepath "$SCRIPT_DIR/src" -d "$STAGE/javadoc" \
  io.github.yearsyan.xpatch >/dev/null
(cd "$STAGE/javadoc" && zip -qr "$DIR/$ARTIFACT-$VERSION-javadoc.jar" .)

cat > "$DIR/$ARTIFACT-$VERSION.pom" <<POM
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <groupId>$GROUP</groupId>
  <artifactId>$ARTIFACT</artifactId>
  <version>$VERSION</version>
  <packaging>aar</packaging>
  <name>$ARTIFACT</name>
  <description>$DESC</description>
  <url>$REPO_URL</url>
  <licenses>
    <license>
      <name>MIT License</name>
      <url>https://opensource.org/licenses/MIT</url>
      <distribution>repo</distribution>
    </license>
  </licenses>
  <developers>
    <developer>
      <id>$PUB_NAME</id>
      <name>$PUB_NAME</name>
      <email>$PUB_EMAIL</email>
    </developer>
  </developers>
  <scm>
    <connection>scm:git:git://github.com/yearsyan/xpatchlib.git</connection>
    <developerConnection>scm:git:ssh://git@github.com/yearsyan/xpatchlib.git</developerConnection>
    <url>$REPO_URL/tree/main</url>
  </scm>
</project>
POM

for f in "$DIR/$ARTIFACT-$VERSION.aar" \
         "$DIR/$ARTIFACT-$VERSION.pom" \
         "$DIR/$ARTIFACT-$VERSION-sources.jar" \
         "$DIR/$ARTIFACT-$VERSION-javadoc.jar"; do
  sign "$f"
  gpg --verify "$f.asc" "$f" >/dev/null 2>&1 || { echo "error: self-verify failed for $f" >&2; exit 1; }
done

(cd "$STAGE" && zip -qr "$OUT" "$GROUP_DIR")
echo "==> bundle: $OUT (signatures self-verified)"
unzip -l "$OUT"

if [[ "${2:-}" == "--upload" ]]; then
  [[ -n "${CENTRAL_USERNAME:-}" && -n "${CENTRAL_PASSWORD:-}" ]] || {
    echo "error: CENTRAL_USERNAME / CENTRAL_PASSWORD not set" >&2; exit 1; }
  echo "==> uploading to Maven Central"
  STATUS=$(curl -sS -o /tmp/central-resp.txt -w '%{http_code}' \
    -u "$CENTRAL_USERNAME:$CENTRAL_PASSWORD" -X POST \
    "https://central.sonatype.com/api/v1/publish?name=$ARTIFACT-$VERSION" \
    --form "bundle=@$OUT")
  if [[ "$STATUS" != 2* ]]; then
    STATUS=$(curl -sS -o /tmp/central-resp.txt -w '%{http_code}' \
      -u "$CENTRAL_USERNAME:$CENTRAL_PASSWORD" -X POST \
      -H 'Content-Type: application/octet-stream' \
      --data-binary "@$OUT" \
      "https://central.sonatype.com/api/v1/publish?name=$ARTIFACT-$VERSION")
  fi
  cat /tmp/central-resp.txt; echo
  [[ "$STATUS" == 2* ]] && echo "==> uploaded (http $STATUS); portal validation runs next" \
                       || { echo "error: upload failed (http $STATUS)" >&2; exit 1; }
fi
