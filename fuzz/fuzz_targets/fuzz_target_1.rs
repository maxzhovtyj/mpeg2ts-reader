#![no_main]

#[macro_use]
extern crate libfuzzer_sys;
#[macro_use]
extern crate mpeg2ts_reader;

use mpeg2ts_reader::{demultiplex, pes, packet};

pub struct FuzzElementaryStreamConsumer;
impl<Ctx> pes::ElementaryStreamConsumer<Ctx> for FuzzElementaryStreamConsumer {
    fn start_stream(&mut self, _ctx: &mut Ctx) {}
    fn begin_packet(&mut self, _ctx: &mut Ctx, _header: pes::PesHeader) {}
    fn continue_packet(&mut self, _ctx: &mut Ctx, _data: &[u8]) {}
    fn end_packet(&mut self, _ctx: &mut Ctx) {}
    fn continuity_error(&mut self, _ctx: &mut Ctx) {}
}

packet_filter_switch!{
    FuzzFilterSwitch<FuzzDemuxContext> {
        Pat: demultiplex::PatPacketFilter<FuzzDemuxContext>,
        Pmt: demultiplex::PmtPacketFilter<FuzzDemuxContext>,
        Elem: pes::PesPacketFilter<FuzzDemuxContext,FuzzElementaryStreamConsumer>,
        Null: demultiplex::NullPacketFilter<FuzzDemuxContext>,
    }
}
demux_context!(FuzzDemuxContext, FuzzFilterSwitch);
impl FuzzDemuxContext {
    fn do_construct(&mut self, req: demultiplex::FilterRequest) -> FuzzFilterSwitch {
        match req {
            demultiplex::FilterRequest::ByPid(packet::Pid::PAT) => FuzzFilterSwitch::Pat(demultiplex::PatPacketFilter::default()),
            demultiplex::FilterRequest::ByPid(_) => FuzzFilterSwitch::Null(demultiplex::NullPacketFilter::default()),
            demultiplex::FilterRequest::ByStream { .. } => FuzzFilterSwitch::Null(demultiplex::NullPacketFilter::default()),
            demultiplex::FilterRequest::Pmt{pid, program_number} => FuzzFilterSwitch::Pmt(demultiplex::PmtPacketFilter::new(pid, program_number)),
            demultiplex::FilterRequest::Nit{pid} => FuzzFilterSwitch::Null(demultiplex::NullPacketFilter::default()),
        }
    }
}
fuzz_target!(|data: &[u8]| {
    let mut ctx = FuzzDemuxContext::new();
    let mut demux = demultiplex::Demultiplex::new(&mut ctx);
    let res = demux.push(&mut ctx, data);
});
