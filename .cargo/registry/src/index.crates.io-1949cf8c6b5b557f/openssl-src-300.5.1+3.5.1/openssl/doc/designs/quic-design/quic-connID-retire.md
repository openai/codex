QUIC Route Requirements
=======================

* Two connection IDs -- one local, one remote

MVP
---

MVP does most of one side of the CID management.  The major outstanding items
for a complete implementation are:

* possibly increase the number of CIDs we permit (from 2)
* use more than the just latest CID for packet transmission
  * round robin non-retired CIDs

Non zero Length Connection ID
-----------------------------

MVP does not issue multiple connection CIDs, instead it uses a zero length CID.
To achieve this, more work is required:

* creation of new CIDs (coded but not used)
* responding to new CIDs by returning new CIDs to peer match
* managing the number of CIDs presented to our peer
* limiting the number of CIDs issued & retired
* retirement of CIDs that are no longer being used
* ensuring only one retire connection ID frame is in flight

Connection Migration
--------------------

* Supporting migration goes well beyond CID management.  The additions required
  to the CID code should be undertaken when/if connection migration is
  supported.  I.e. do this later in a just in time manner.

Retiring Connection ID
----------------------

When a remote asks to retire a connection ID (RETIRE_CONNECTION_ID) we have to:

* Send retirement acks for all retired CIDs
* Immediately delete all CIDs and routes associated with these CIDs
  * Retransmits use different route, so they are good.
  * Out of order delivery will initiate retransmits
* Should respond with a NEW_CONNECTION_ID frame if we are low on CIDs
  * Not sure if it is mandatory to send a retirement.

When a remote creates a new connection ID:

* May respond with a new connection ID frame (it's a good idea)
* It reads like the NEW_CONNECTION_ID frame can't be used to retire routes.
  However, see above.  Suggest we accept either.

When we want to retire one (or more) connection IDs we have to:

* Flag the route(s) as retired
* Send a retirement frame (RETIRE_CONNECTION_ID)
* Delete the connection(s) once they are retired by our peer (either
  NEW_CONNECTION_ID or RETIRE_CONNECTION_ID can do this)

State
-----

* routes we've retired until they are acked as being retired (uint64_t max CID)
* routes our peer has retired don't need tracking, we can remove immediately
* retired routes where we've outstanding data to send will have that data
  sent before the retirement acks are send.  If these fragments need to
  be retransmitted, they'll be done so using a new CID on a new route.
  This means, there is no requirement to wait for data to be flushed
  before sending the retirement ack.
