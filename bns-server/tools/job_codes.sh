#!/bin/sh
# per method: last argument of each Send*Fail, plus Set() tasks and case lists
awk '
/__fastcall/ { if (m) print m ": " s; m=$0; sub(/\(.*/,"",m); sub(/.*::/,"",m); s="" }
/Send[A-Za-z]*Fail\(/ { x=$0; sub(/\);.*$/,"",x); n=split(x,a,", "); s=s " F(" a[n] ")" }
/Send[A-Za-z]*Ok\(/ { s=s " OK" }
/LobbyJobTransaction::Set\(|_task = [0-9]+;/ { x=$0; gsub(/[^0-9]+/," ",x); n=split(x,a," "); s=s " tran" a[n] }
/^ *case / { x=$1 $2; s=s " " x }
/transit\(/ { x=$0; sub(/\);.*$/,"",x); n=split(x,a,", "); s=s " ->" a[n] }
/^ *(v[0-9]+|reason) = [0-9]+;$/ { s=s " " $3 }
END { print m ": " s }' "$1" | cut -c1-260
