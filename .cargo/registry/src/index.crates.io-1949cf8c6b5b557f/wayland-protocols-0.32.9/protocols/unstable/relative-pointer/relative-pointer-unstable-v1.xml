<?xml version="1.0" encoding="UTF-8"?>
<protocol name="relative_pointer_unstable_v1">

  <copyright>
    Copyright © 2014      Jonas Ådahl
    Copyright © 2015      Red Hat Inc.

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

  <description summary="protocol for relative pointer motion events">
    This protocol specifies a set of interfaces used for making clients able to
    receive relative pointer events not obstructed by barriers (such as the
    monitor edge or other pointer barriers).

    To start receiving relative pointer events, a client must first bind the
    global interface "wp_relative_pointer_manager" which, if a compositor
    supports relative pointer motion events, is exposed by the registry. After
    having created the relative pointer manager proxy object, the client uses
    it to create the actual relative pointer object using the
    "get_relative_pointer" request given a wl_pointer. The relative pointer
    motion events will then, when applicable, be transmitted via the proxy of
    the newly created relative pointer object. See the documentation of the
    relative pointer interface for more details.

    Warning! The protocol described in this file is experimental and backward
    incompatible changes may be made. Backward compatible changes may be added
    together with the corresponding interface version bump. Backward
    incompatible changes are done by bumping the version number in the protocol
    and interface names and resetting the interface version. Once the protocol
    is to be declared stable, the 'z' prefix and the version number in the
    protocol and interface names are removed and the interface version number is
    reset.
  </description>

  <interface name="zwp_relative_pointer_manager_v1" version="1">
    <description summary="get relative pointer objects">
      A global interface used for getting the relative pointer object for a
      given pointer.
    </description>

    <request name="destroy" type="destructor">
      <description summary="destroy the relative pointer manager object">
	Used by the client to notify the server that it will no longer use this
	relative pointer manager object.
      </description>
    </request>

    <request name="get_relative_pointer">
      <description summary="get a relative pointer object">
	Create a relative pointer interface given a wl_pointer object. See the
	wp_relative_pointer interface for more details.
      </description>
      <arg name="id" type="new_id" interface="zwp_relative_pointer_v1"/>
      <arg name="pointer" type="object" interface="wl_pointer"/>
    </request>
  </interface>

  <interface name="zwp_relative_pointer_v1" version="1">
    <description summary="relative pointer object">
      A wp_relative_pointer object is an extension to the wl_pointer interface
      used for emitting relative pointer events. It shares the same focus as
      wl_pointer objects of the same seat and will only emit events when it has
      focus.
    </description>

    <request name="destroy" type="destructor">
      <description summary="release the relative pointer object"/>
    </request>

    <event name="relative_motion">
      <description summary="relative pointer motion">
	Relative x/y pointer motion from the pointer of the seat associated with
	this object.

	A relative motion is in the same dimension as regular wl_pointer motion
	events, except they do not represent an absolute position. For example,
	moving a pointer from (x, y) to (x', y') would have the equivalent
	relative motion (x' - x, y' - y). If a pointer motion caused the
	absolute pointer position to be clipped by for example the edge of the
	monitor, the relative motion is unaffected by the clipping and will
	represent the unclipped motion.

	This event also contains non-accelerated motion deltas. The
	non-accelerated delta is, when applicable, the regular pointer motion
	delta as it was before having applied motion acceleration and other
	transformations such as normalization.

	Note that the non-accelerated delta does not represent 'raw' events as
	they were read from some device. Pointer motion acceleration is device-
	and configuration-specific and non-accelerated deltas and accelerated
	deltas may have the same value on some devices.

	Relative motions are not coupled to wl_pointer.motion events, and can be
	sent in combination with such events, but also independently. There may
	also be scenarios where wl_pointer.motion is sent, but there is no
	relative motion. The order of an absolute and relative motion event
	originating from the same physical motion is not guaranteed.

	If the client needs button events or focus state, it can receive them
	from a wl_pointer object of the same seat that the wp_relative_pointer
	object is associated with.
      </description>
      <arg name="utime_hi" type="uint"
	   summary="high 32 bits of a 64 bit timestamp with microsecond granularity"/>
      <arg name="utime_lo" type="uint"
	   summary="low 32 bits of a 64 bit timestamp with microsecond granularity"/>
      <arg name="dx" type="fixed"
	   summary="the x component of the motion vector"/>
      <arg name="dy" type="fixed"
	   summary="the y component of the motion vector"/>
      <arg name="dx_unaccel" type="fixed"
	   summary="the x component of the unaccelerated motion vector"/>
      <arg name="dy_unaccel" type="fixed"
	   summary="the y component of the unaccelerated motion vector"/>
    </event>
  </interface>

</protocol>
