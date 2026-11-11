#!/usr/bin/perl
# Put a wall-clock time (HH:MM:SS.mmm) before each input line. Used by tools/run_play.sh for the service logs.
use strict; use warnings; use Time::HiRes qw(time); use POSIX qw(strftime);
$| = 1;
while (my $line = <STDIN>) {
    my $t = time;
    printf "%s.%03d %s", strftime("%H:%M:%S", localtime $t), ($t - int $t) * 1000, $line;
}
