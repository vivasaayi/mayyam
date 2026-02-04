#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "Usage: $0 path/to/junit.xml"
  exit 2
fi

file=$1
if [ ! -f "$file" ]; then
  echo "File not found: $file" >&2
  exit 2
fi

echo "Parsing junit file: $file"

# Use xmllint if available for easy parsing; otherwise fallback to python xml parsing
if command -v xmllint >/dev/null 2>&1; then
  fails=$(xmllint --xpath 'string(//testsuite/@failures)' "$file" 2>/dev/null || echo 0)
  errors=$(xmllint --xpath 'string(//testsuite/@errors)' "$file" 2>/dev/null || echo 0)
  tests=$(xmllint --xpath 'string(//testsuite/@tests)' "$file" 2>/dev/null || echo 0)
else
  read_val=$(python - <<PY
import sys,xml.etree.ElementTree as ET
root=ET.parse(sys.argv[1]).getroot()
tests=root.attrib.get('tests','0')
fails=root.attrib.get('failures','0')
errs=root.attrib.get('errors','0')
print('%s %s %s' % (tests,fails,errs))
PY
  tests=$(echo $read_val | awk '{print $1}')
  fails=$(echo $read_val | awk '{print $2}')
  errors=$(echo $read_val | awk '{print $3}')
fi

echo "JUnit: tests=$tests failures=$fails errors=$errors"
if [ "$fails" != "0" ] || [ "$errors" != "0" ]; then
  echo "Integration tests had failures/errors" >&2
  exit 1
fi

echo "Integration junit tests succeeded"
exit 0
