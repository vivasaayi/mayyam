package org.poriyiyal.mayyam.ticketmanagement.repository;

import org.poriyiyal.mayyam.ticketmanagement.entity.Ticket;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

@Repository
public interface TicketRepository extends JpaRepository<Ticket, Long> {
}