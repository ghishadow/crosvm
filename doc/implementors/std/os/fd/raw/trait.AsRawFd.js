(function() {var implementors = {};
implementors["base"] = [{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"base/struct.DescriptorAdapter.html\" title=\"struct base::DescriptorAdapter\">DescriptorAdapter</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"base/trait.DescriptorIntoAsync.html\" title=\"trait base::DescriptorIntoAsync\">DescriptorIntoAsync</a>,&nbsp;</span>","synthetic":false,"types":["base::async_types::DescriptorAdapter"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"base/struct.Tube.html\" title=\"struct base::Tube\">Tube</a>","synthetic":false,"types":["base::tube::Tube"]}];
implementors["cros_async"] = [{"text":"impl&lt;T:&nbsp;<a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"cros_async/struct.AsyncWrapper.html\" title=\"struct cros_async::AsyncWrapper\">AsyncWrapper</a>&lt;T&gt;","synthetic":false,"types":["cros_async::io_ext::AsyncWrapper"]}];
implementors["io_uring"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"io_uring/struct.URingContext.html\" title=\"struct io_uring::URingContext\">URingContext</a>","synthetic":false,"types":["io_uring::uring::URingContext"]}];
implementors["net_util"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"net_util/struct.Tap.html\" title=\"struct net_util::Tap\">Tap</a>","synthetic":false,"types":["net_util::Tap"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"net_util/fakes/struct.FakeTap.html\" title=\"struct net_util::fakes::FakeTap\">FakeTap</a>","synthetic":false,"types":["net_util::fakes::FakeTap"]}];
implementors["sys_util"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.SafeDescriptor.html\" title=\"struct sys_util::SafeDescriptor\">SafeDescriptor</a>","synthetic":false,"types":["sys_util::descriptor::SafeDescriptor"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.Descriptor.html\" title=\"struct sys_util::Descriptor\">Descriptor</a>","synthetic":false,"types":["sys_util::descriptor::Descriptor"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.EventFd.html\" title=\"struct sys_util::EventFd\">EventFd</a>","synthetic":false,"types":["sys_util::eventfd::EventFd"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/net/struct.TcpSocket.html\" title=\"struct sys_util::net::TcpSocket\">TcpSocket</a>","synthetic":false,"types":["sys_util::net::TcpSocket"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/net/struct.UnixSeqpacket.html\" title=\"struct sys_util::net::UnixSeqpacket\">UnixSeqpacket</a>","synthetic":false,"types":["sys_util::net::UnixSeqpacket"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for &amp;<a class=\"struct\" href=\"sys_util/net/struct.UnixSeqpacket.html\" title=\"struct sys_util::net::UnixSeqpacket\">UnixSeqpacket</a>","synthetic":false,"types":["sys_util::net::UnixSeqpacket"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/net/struct.UnixSeqpacketListener.html\" title=\"struct sys_util::net::UnixSeqpacketListener\">UnixSeqpacketListener</a>","synthetic":false,"types":["sys_util::net::UnixSeqpacketListener"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/net/struct.UnlinkUnixSeqpacketListener.html\" title=\"struct sys_util::net::UnlinkUnixSeqpacketListener\">UnlinkUnixSeqpacketListener</a>","synthetic":false,"types":["sys_util::net::UnlinkUnixSeqpacketListener"]},{"text":"impl&lt;T:&nbsp;<a class=\"trait\" href=\"sys_util/trait.PollToken.html\" title=\"trait sys_util::PollToken\">PollToken</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.EpollContext.html\" title=\"struct sys_util::EpollContext\">EpollContext</a>&lt;T&gt;","synthetic":false,"types":["sys_util::poll::EpollContext"]},{"text":"impl&lt;T:&nbsp;<a class=\"trait\" href=\"sys_util/trait.PollToken.html\" title=\"trait sys_util::PollToken\">PollToken</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.PollContext.html\" title=\"struct sys_util::PollContext\">PollContext</a>&lt;T&gt;","synthetic":false,"types":["sys_util::poll::PollContext"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.Fd.html\" title=\"struct sys_util::Fd\">Fd</a>","synthetic":false,"types":["sys_util::raw_fd::Fd"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.SharedMemory.html\" title=\"struct sys_util::SharedMemory\">SharedMemory</a>","synthetic":false,"types":["sys_util::shm::SharedMemory"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for &amp;<a class=\"struct\" href=\"sys_util/struct.SharedMemory.html\" title=\"struct sys_util::SharedMemory\">SharedMemory</a>","synthetic":false,"types":["sys_util::shm::SharedMemory"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.SignalFd.html\" title=\"struct sys_util::SignalFd\">SignalFd</a>","synthetic":false,"types":["sys_util::signalfd::SignalFd"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.TimerFd.html\" title=\"struct sys_util::TimerFd\">TimerFd</a>","synthetic":false,"types":["sys_util::timerfd::TimerFd"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/struct.FakeTimerFd.html\" title=\"struct sys_util::FakeTimerFd\">FakeTimerFd</a>","synthetic":false,"types":["sys_util::timerfd::FakeTimerFd"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/vsock/struct.VsockSocket.html\" title=\"struct sys_util::vsock::VsockSocket\">VsockSocket</a>","synthetic":false,"types":["sys_util::vsock::VsockSocket"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/vsock/struct.VsockStream.html\" title=\"struct sys_util::vsock::VsockStream\">VsockStream</a>","synthetic":false,"types":["sys_util::vsock::VsockStream"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"sys_util/vsock/struct.VsockListener.html\" title=\"struct sys_util::vsock::VsockListener\">VsockListener</a>","synthetic":false,"types":["sys_util::vsock::VsockListener"]}];
implementors["tempfile"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"tempfile/struct.NamedTempFile.html\" title=\"struct tempfile::NamedTempFile\">NamedTempFile</a>","synthetic":false,"types":["tempfile::file::NamedTempFile"]}];
implementors["vmm_vhost"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"vmm_vhost/connection/socket/struct.Listener.html\" title=\"struct vmm_vhost::connection::socket::Listener\">Listener</a>","synthetic":false,"types":["vmm_vhost::connection::socket::Listener"]},{"text":"impl&lt;T:&nbsp;<a class=\"trait\" href=\"vmm_vhost/message/trait.Req.html\" title=\"trait vmm_vhost::message::Req\">Req</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"vmm_vhost/connection/socket/struct.Endpoint.html\" title=\"struct vmm_vhost::connection::socket::Endpoint\">Endpoint</a>&lt;T&gt;","synthetic":false,"types":["vmm_vhost::connection::socket::Endpoint"]},{"text":"impl&lt;E:&nbsp;<a class=\"trait\" href=\"vmm_vhost/connection/trait.Endpoint.html\" title=\"trait vmm_vhost::connection::Endpoint\">Endpoint</a>&lt;<a class=\"enum\" href=\"vmm_vhost/message/enum.MasterReq.html\" title=\"enum vmm_vhost::message::MasterReq\">MasterReq</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"vmm_vhost/struct.Master.html\" title=\"struct vmm_vhost::Master\">Master</a>&lt;E&gt;","synthetic":false,"types":["vmm_vhost::master::Master"]},{"text":"impl&lt;S:&nbsp;<a class=\"trait\" href=\"vmm_vhost/trait.VhostUserMasterReqHandler.html\" title=\"trait vmm_vhost::VhostUserMasterReqHandler\">VhostUserMasterReqHandler</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"vmm_vhost/struct.MasterReqHandler.html\" title=\"struct vmm_vhost::MasterReqHandler\">MasterReqHandler</a>&lt;S&gt;","synthetic":false,"types":["vmm_vhost::master_req_handler::MasterReqHandler"]},{"text":"impl&lt;S:&nbsp;<a class=\"trait\" href=\"vmm_vhost/trait.VhostUserSlaveReqHandler.html\" title=\"trait vmm_vhost::VhostUserSlaveReqHandler\">VhostUserSlaveReqHandler</a>, E:&nbsp;<a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> + <a class=\"trait\" href=\"vmm_vhost/connection/trait.Endpoint.html\" title=\"trait vmm_vhost::connection::Endpoint\">Endpoint</a>&lt;<a class=\"enum\" href=\"vmm_vhost/message/enum.MasterReq.html\" title=\"enum vmm_vhost::message::MasterReq\">MasterReq</a>&gt;&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.56.1/std/os/fd/raw/trait.AsRawFd.html\" title=\"trait std::os::fd::raw::AsRawFd\">AsRawFd</a> for <a class=\"struct\" href=\"vmm_vhost/struct.SlaveReqHandler.html\" title=\"struct vmm_vhost::SlaveReqHandler\">SlaveReqHandler</a>&lt;S, E&gt;","synthetic":false,"types":["vmm_vhost::slave_req_handler::SlaveReqHandler"]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()