#! /usr/bin/env perl -pi

BEGIN {
    our $count = 1;              # Only the first one
    our $RELEASE = $ENV{RELEASE};
    our $RELEASE_TEXT = $ENV{RELEASE_TEXT};
    our $PREV_RELEASE_DATE = $ENV{PREV_RELEASE_DATE} || 'under development';
    our $PREV_RELEASE_TEXT = $ENV{PREV_RELEASE_TEXT};

    $RELEASE =~ s/-dev//;
}

if (/^### Major changes between OpenSSL (\S+) and OpenSSL (\S+) \[under development\]/
    && $count-- > 0) {
    my $v1 = $1;
    my $v2 = $PREV_RELEASE_TEXT || $2;

    # If this is a pre-release, we do nothing
    if ($RELEASE !~ /^\d+\.\d+\.\d+-(?:alpha|beta)/) {
        $_ = <<_____
### Major changes between OpenSSL $v2 and OpenSSL $RELEASE_TEXT [under development]

  * none

### Major changes between OpenSSL $v1 and OpenSSL $v2 [$PREV_RELEASE_DATE]
_____
    }
}
