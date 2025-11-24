<?php

// php -S localhost:8000 server.php
date_default_timezone_set('Europe/Paris');

if (!str_contains($_SERVER['HTTP_ACCEPT'] ?? '', 'text/event-stream')):
?>
Test EventSource
<script>
    const es = new EventSource(window.location.href);
    es.onmessage = (e) => console.log(e);
    es.onerror = () => es.close();
</script>
<?php
    return;

endif;

header('Cache-Control: no-store');
header('Content-Type: text/event-stream');


echo <<<EOF



event: message\r\ndata:line1
data: line2
:
id: my-id
:should be ignored too\rretry:42




data: second

event:
data: third

event:  
data: fourth

EOF;
