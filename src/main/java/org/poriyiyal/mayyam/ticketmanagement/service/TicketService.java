package org.poriyiyal.mayyam.ticketmanagement.service;

import org.poriyiyal.mayyam.ticketmanagement.entity.Ticket;
import org.poriyiyal.mayyam.ticketmanagement.repository.TicketRepository;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

import java.util.List;
import java.util.Optional;

@Service
public class TicketService {

    @Autowired
    private TicketRepository ticketRepository;

    public List<Ticket> getAllTickets() {
        return ticketRepository.findAll();
    }

    public Optional<Ticket> getTicketById(Long id) {
        return ticketRepository.findById(id);
    }

    public Ticket createTicket(Ticket ticket) {
        return ticketRepository.save(ticket);
    }

    public Ticket updateTicket(Long id, Ticket ticketDetails) {
        Ticket ticket = ticketRepository.findById(id).orElseThrow(() -> new RuntimeException("Ticket not found"));
        ticket.setTitle(ticketDetails.getTitle());
        ticket.setDescription(ticketDetails.getDescription());
        ticket.setStatus(ticketDetails.getStatus());
        ticket.setExternalSystem(ticketDetails.getExternalSystem());
        ticket.setExternalTicketId(ticketDetails.getExternalTicketId());
        ticket.setArn(ticketDetails.getArn());
        ticket.setResourceName(ticketDetails.getResourceName());
        ticket.setResourceType(ticketDetails.getResourceType());
        return ticketRepository.save(ticket);
    }

    public void deleteTicket(Long id) {
        Ticket ticket = ticketRepository.findById(id).orElseThrow(() -> new RuntimeException("Ticket not found"));
        ticketRepository.delete(ticket);
    }
}