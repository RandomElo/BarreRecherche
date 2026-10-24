import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { Reglages } from './reglages/Reglages';
import './styles.css';

const racine = document.getElementById('racine');
if (racine === null) {
  throw new Error('Élément #racine introuvable dans reglages.html.');
}
createRoot(racine).render(
  <StrictMode>
    <Reglages />
  </StrictMode>,
);
