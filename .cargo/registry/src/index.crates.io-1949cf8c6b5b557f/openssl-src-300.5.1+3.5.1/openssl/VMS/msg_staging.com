$       ! Used by the main descrip.mms to print the statging installation
$       ! complete
$       ! message.
$       ! Arguments:
$       ! P1    staging software installation directory
$       ! P2    staging data installation directory
$       ! P3    final software installation directory
$       ! P4    final data installation directory
$       ! P5    startup / setup / shutdown scripts directory
$       ! P6    distinguishing version number ("major version")
$
$       staging_instdir = p1
$       staging_datadir = p2
$       final_instdir = p3
$       final_datadir = p4
$       systartup = p5
$       osslver = p6
$
$       WRITE SYS$OUTPUT "Staging installation complete"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "Finish or package in such a way that the contents of the following directory"
$       WRITE SYS$OUTPUT "trees end up being copied:"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "- from ", staging_instdir
$       WRITE SYS$OUTPUT "  to   ", final_instdir
$       WRITE SYS$OUTPUT "- from ", staging_datadir
$       WRITE SYS$OUTPUT "  to   ", final_datadir
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "When in its final destination, the following commands need to be executed"
$       WRITE SYS$OUTPUT "to use OpenSSL:"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "- to set up OpenSSL logical names:"
$       WRITE SYS$OUTPUT "  @''systartup'openssl_startup''osslver'"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "- to define the OpenSSL command"
$       WRITE SYS$OUTPUT "  @''systartup'openssl_utils''osslver'"
$       WRITE SYS$OUTPUT ""
