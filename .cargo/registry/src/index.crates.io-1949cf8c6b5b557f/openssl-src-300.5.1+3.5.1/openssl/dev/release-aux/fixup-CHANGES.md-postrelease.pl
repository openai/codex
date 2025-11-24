#! /usr/bin/env perl -pi

BEGIN {
    our $count = 1;              # Only the first one
    our $RELEASE = $ENV{RELEASE};
    our $RELEASE_TEXT = $ENV{RELEASE_TEXT};
    our $PREV_RELEASE_DATE = $ENV{PREV_RELEASE_DATE} || 'xx XXX xxxx';
    our $PREV_RELEASE_TEXT = $ENV{PREV_RELEASE_TEXT};

    $RELEASE =~ s/-dev//;
}

if (/^### Changes between (\S+) and (\S+) \[xx XXX xxxx\]/
    && $count-- > 0) {
    my $v1 = $1;
    my $v2 = $PREV_RELEASE_TEXT || $2;

    # If this is a pre-release, we do nothing
    if ($RELEASE !~ /^\d+\.\d+\.\d+-(?:alpha|beta)/) {
        $_ = <<_____
### Changes between $v2 and $RELEASE_TEXT [xx XXX xxxx]

 * none yet

### Changes between $v1 and $v2 [$PREV_RELEASE_DATE]
_____
    }
}
