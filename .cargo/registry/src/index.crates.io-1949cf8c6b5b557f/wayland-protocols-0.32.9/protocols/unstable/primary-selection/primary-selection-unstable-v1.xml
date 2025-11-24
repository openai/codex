<?xml version="1.0" encoding="UTF-8"?>
<protocol name="wp_primary_selection_unstable_v1">
  <copyright>
    Copyright © 2015, 2016 Red Hat

    Permission is hereby granted, free of charge, to any person obtaining a
    copy of this software and associated documentation files (the "Software"),
    to deal in the Software without restriction, including without limitation
    the rights to use, copy, modify, merge, publish, distribute, sublicense,
    and/or sell copies of the Software, and to permit persons to whom the
    Software is furnished to do so, subject to the following conditions:

    The above copyright notice and this permission notice (including the next
    paragraph) shall be included in all copies or substantial portions of the
    Software.

    THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL
    THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
    FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
    DEALINGS IN THE SOFTWARE.
  </copyright>

  <description summary="Primary selection protocol">
    This protocol provides the ability to have a primary selection device to
    match that of the X server. This primary selection is a shortcut to the
    common clipboard selection, where text just needs to be selected in order
    to allow copying it elsewhere. The de facto way to perform this action
    is the middle mouse button, although it is not limited to this one.

    Clients wishing to honor primary selection should create a primary
    selection source and set it as the selection through
    wp_primary_selection_device.set_selection whenever the text selection
    changes. In order to minimize calls in pointer-driven text selection,
    it should happen only once after the operation finished. Similarly,
    a NULL source should be set when text is unselected.

    wp_primary_selection_offer objects are first announced through the
    wp_primary_selection_device.data_offer event. Immediately after this event,
    the primary data offer will emit wp_primary_selection_offer.offer events
    to let know of the mime types being offered.

    When the primary selection changes, the client with the keyboard focus
    will receive wp_primary_selection_device.selection events. Only the client
    with the keyboard focus will receive such events with a non-NULL
    wp_primary_selection_offer. Across keyboard focus changes, previously
    focused clients will receive wp_primary_selection_device.events with a
    NULL wp_primary_selection_offer.

    In order to request the primary selection data, the client must pass
    a recent serial pertaining to the press event that is triggering the
    operation, if the compositor deems the serial valid and recent, the
    wp_primary_selection_source.send event will happen in the other end
    to let the transfer begin. The client owning the primary selection
    should write the requested data, and close the file descriptor
    immediately.

    If the primary selection owner client disappeared during the transfer,
    the client reading the data will receive a
    wp_primary_selection_device.selection event with a NULL
    wp_primary_selection_offer, the client should take this as a hint
    to finish the reads related to the no longer existing offer.

    The primary selection owner should be checking for errors during
    writes, merely cancelling the ongoing transfer if any happened.
  </description>

  <interface name="zwp_primary_selection_device_manager_v1" version="1">
    <description summary="X primary selection emulation">
      The primary selection device manager is a singleton global object that
      provides access to the primary selection. It allows to create
      wp_primary_selection_source objects, as well as retrieving the per-seat
      wp_primary_selection_device objects.
    </description>

    <request name="create_source">
      <description summary="create a new primary selection source">
        Create a new primary selection source.
      </description>
      <arg name="id" type="new_id" interface="zwp_primary_selection_source_v1"/>
    </request>

    <request name="get_device">
      <description summary="create a new primary selection device">
        Create a new data device for a given seat.
      </description>
      <arg name="id" type="new_id" interface="zwp_primary_selection_device_v1"/>
      <arg name="seat" type="object" interface="wl_seat"/>
    </request>

    <request name="destroy" type="destructor">
      <description summary="destroy the primary selection device manager">
        Destroy the primary selection device manager.
      </description>
    </request>
  </interface>

  <interface name="zwp_primary_selection_device_v1" version="1">
    <request name="set_selection">
      <description summary="set the primary selection">
        Replaces the current selection. The previous owner of the primary
        selection will receive a wp_primary_selection_source.cancelled event.

        To unset the selection, set the source to NULL.
      </description>
      <arg name="source" type="object" interface="zwp_primary_selection_source_v1" allow-null="true"/>
      <arg name="serial" type="uint" summary="serial of the event that triggered this request"/>
    </request>

    <event name="data_offer">
      <description summary="introduce a new wp_primary_selection_offer">
        Introduces a new wp_primary_selection_offer object that may be used
        to receive the current primary selection. Immediately following this
        event, the new wp_primary_selection_offer object will send
        wp_primary_selection_offer.offer events to describe the offered mime
        types.
      </description>
      <arg name="offer" type="new_id" interface="zwp_primary_selection_offer_v1"/>
    </event>

    <event name="selection">
      <description summary="advertise a new primary selection">
        The wp_primary_selection_device.selection event is sent to notify the
        client of a new primary selection. This event is sent after the
        wp_primary_selection.data_offer event introducing this object, and after
        the offer has announced its mimetypes through
        wp_primary_selection_offer.offer.

        The data_offer is valid until a new offer or NULL is received
        or until the client loses keyboard focus. The client must destroy the
        previous selection data_offer, if any, upon receiving this event.
      </description>
      <arg name="id" type="object" interface="zwp_primary_selection_offer_v1" allow-null="true"/>
    </event>

    <request name="destroy" type="destructor">
      <description summary="destroy the primary selection device">
        Destroy the primary selection device.
      </description>
    </request>
  </interface>

  <interface name="zwp_primary_selection_offer_v1" version="1">
    <description summary="offer to transfer primary selection contents">
      A wp_primary_selection_offer represents an offer to transfer the contents
      of the primary selection clipboard to the client. Similar to
      wl_data_offer, the offer also describes the mime types that the data can
      be converted to and provides the mechanisms for transferring the data
      directly to the client.
    </description>

    <request name="receive">
      <description summary="request that the data is transferred">
        To transfer the contents of the primary selection clipboard, the client
        issues this request and indicates the mime type that it wants to
        receive. The transfer happens through the passed file descriptor
        (typically created with the pipe system call). The source client writes
        the data in the mime type representation requested and then closes the
        file descriptor.

        The receiving client reads from the read end of the pipe until EOF and
        closes its end, at which point the transfer is complete.
      </description>
      <arg name="mime_type" type="string"/>
      <arg name="fd" type="fd"/>
    </request>

    <request name="destroy" type="destructor">
      <description summary="destroy the primary selection offer">
        Destroy the primary selection offer.
      </description>
    </request>

    <event name="offer">
      <description summary="advertise offered mime type">
        Sent immediately after creating announcing the
        wp_primary_selection_offer through
        wp_primary_selection_device.data_offer. One event is sent per offered
        mime type.
      </description>
      <arg name="mime_type" type="string"/>
    </event>
  </interface>

  <interface name="zwp_primary_selection_source_v1" version="1">
    <description summary="offer to replace the contents of the primary selection">
      The source side of a wp_primary_selection_offer, it provides a way to
      describe the offered data and respond to requests to transfer the
      requested contents of the primary selection clipboard.
    </description>

    <request name="offer">
      <description summary="add an offered mime type">
        This request adds a mime type to the set of mime types advertised to
        targets. Can be called several times to offer multiple types.
      </description>
      <arg name="mime_type" type="string"/>
    </request>

    <request name="destroy" type="destructor">
      <description summary="destroy the primary selection source">
        Destroy the primary selection source.
      </description>
    </request>

    <event name="send">
      <description summary="send the primary selection contents">
        Request for the current primary selection contents from the client.
        Send the specified mime type over the passed file descriptor, then
        close it.
      </description>
      <arg name="mime_type" type="string"/>
      <arg name="fd" type="fd"/>
    </event>

    <event name="cancelled">
      <description summary="request for primary selection contents was canceled">
        This primary selection source is no longer valid. The client should
        clean up and destroy this primary selection source.
      </description>
    </event>
  </interface>
</protocol>
