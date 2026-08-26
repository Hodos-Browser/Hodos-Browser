{
  n++; b = length($0) + 2; bytes += b
  lvl="OTHER"; proc="OTHER"
  if (match($0, /\[(MAIN|RENDER|BROWSER|UNKNOWN)\] \[(DEBUG|INFO|WARN|ERROR)\]/)) {
    s = substr($0, RSTART, RLENGTH)
    c = index(s, "]")
    proc = substr(s, 2, c - 2)
    r = substr(s, c + 3)
    lvl = substr(r, 1, index(r, "]") - 1)
  }
  L[lvl]++; LB[lvl] += b
  P[proc]++
  if (index($0,"Resource request:")) { rr++; rrb += b }
  else if (index($0,"Method: GET, Connection")) { mg++; mgb += b }
  else if (index($0,"Message received:")) { mr++; mrb += b }
  if (index($0,"NEW SESSION STARTED")) sess++
  if (index($0,"freopen failed")) fo++
  if (index($0,"ProfileManager initializing")) pmi++
}
END {
  printf "lines=%d bytes=%d sessions=%d freopen_failed=%d profilemgr_init_cout=%d\n", n, bytes, sess, fo, pmi
  for (k in L) printf "LEVEL %-6s %12d lines %14d bytes (%5.1f%% of bytes)\n", k, L[k], LB[k], 100*LB[k]/bytes
  for (k in P) printf "PROC  %-8s %12d lines\n", k, P[k]
  printf "PATTERN resource_request %10d lines %14d bytes (%5.1f%%)\n", rr, rrb, 100*rrb/bytes
  printf "PATTERN method_get       %10d lines %14d bytes (%5.1f%%)\n", mg, mgb, 100*mgb/bytes
  printf "PATTERN message_received %10d lines %14d bytes (%5.1f%%)\n", mr, mrb, 100*mrb/bytes
}
