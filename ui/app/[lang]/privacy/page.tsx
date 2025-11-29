import type { Metadata } from "next";

type Section = {
  id: string;
  titleDe: string;
  titleEn: string;
  pointsDe: string[];
  pointsEn: string[];
};

const sections: Section[] = [
  {
    id: "controller",
    titleDe: "Verantwortlicher",
    titleEn: "Controller",
    pointsDe: [
      "TQDM Inc., 1111B S Governors Ave, STE 23256, Dover, DE 19904, USA",
      "E-Mail: contact@tqdm.org | Telefon: +1 (814) 524-5685"
    ],
    pointsEn: [
      "TQDM Inc., 1111B S Governors Ave, STE 23256, Dover, DE 19904, USA",
      "Email: contact@tqdm.org | Phone: +1 (814) 524-5685"
    ]
  },
  {
    id: "data",
    titleDe: "Verarbeitete Daten",
    titleEn: "Data processed",
    pointsDe: [
      "Ihre E-Mail-Adresse",
      "Die von Ihnen eingegebene Vorgangsnummer (Pass-, Ausweis- oder eID-Vorgang)",
      "Abgerufene Statusinformationen der Landeshauptstadt München",
      "Technische Server-Protokolle (z. B. IP-Adresse, Zeitpunkt, Fehlermeldungen)",
      "Keine Tracking-Cookies, kein Analytics, keine Werbedaten"
    ],
    pointsEn: [
      "Your email address",
      "The process number you enter (passport, ID, or eID case)",
      "Retrieved status information from the City of Munich systems",
      "Technical server logs (e.g., IP address, timestamp, error messages)",
      "No tracking cookies, no analytics, no advertising data"
    ]
  },
  {
    id: "purpose",
    titleDe: "Zweck und Rechtsgrundlage",
    titleEn: "Purpose and legal basis",
    pointsDe: [
      "Status abrufen, anzeigen und per E-Mail über Änderungen informieren",
      "Technische Funktionsfähigkeit und Sicherheit gewährleisten",
      "Rechtsgrundlage: Art. 6 Abs. 1 lit. b DSGVO (Bereitstellung des Dienstes)",
      "Für technische Protokolle zusätzlich Art. 6 Abs. 1 lit. f DSGVO (berechtigtes Interesse Betrieb/Sicherheit)"
    ],
    pointsEn: [
      "Retrieve and display status updates; notify you via email about changes",
      "Maintain technical functionality and security",
      "Legal basis: Art. 6(1)(b) GDPR (providing the service)",
      "For technical logs also Art. 6(1)(f) GDPR (legitimate interest in operation/security)"
    ]
  },
  {
    id: "retention",
    titleDe: "Speicherung und Löschung",
    titleEn: "Storage and deletion",
    pointsDe: [
      "Daten werden gespeichert, solange Sie den Dienst nutzen.",
      "Nach Beendigung oder Löschung eines Vorgangs werden die Daten nach angemessener Zeit gelöscht.",
      "Server-Protokolle werden nach kurzer Frist automatisch gelöscht."
    ],
    pointsEn: [
      "Data is stored while you use the service.",
      "When you stop using it or delete a case, data is deleted after a reasonable period.",
      "Server logs are automatically deleted after a short retention."
    ]
  },
  {
    id: "sharing",
    titleDe: "Weitergabe von Daten",
    titleEn: "Sharing of data",
    pointsDe: [
      "Keine Weitergabe an Dritte, außer an technische Dienstleister (Hosting, E-Mail) zur Bereitstellung des Dienstes.",
      "Einsatz nur auf Basis von Art. 28 DSGVO (Auftragsverarbeitung).",
      "Keine Werbung, kein Tracking, kein Verkauf von Daten."
    ],
    pointsEn: [
      "No disclosure to third parties except technical providers (hosting, email) required to deliver the service.",
      "Used only under Art. 28 GDPR (data processing agreements).",
      "No advertising, no tracking, no sale of data."
    ]
  },
  {
    id: "usa",
    titleDe: "Datenübermittlung in die USA",
    titleEn: "Transfers to the USA",
    pointsDe: [
      "Als US-Unternehmen kann eine Übermittlung in die USA erfolgen.",
      "Rechtsgrundlage: EU-Standardvertragsklauseln (SCCs) plus ergänzende technische und organisatorische Maßnahmen."
    ],
    pointsEn: [
      "As a US company, transfers to the USA may occur.",
      "Based on EU Standard Contractual Clauses (SCCs) plus supplementary technical and organizational measures."
    ]
  },
  {
    id: "rights",
    titleDe: "Ihre Rechte",
    titleEn: "Your rights",
    pointsDe: [
      "Auskunft, Berichtigung, Löschung, Einschränkung, Datenübertragbarkeit (Art. 15–20 DSGVO)",
      "Widerspruch gegen bestimmte Verarbeitungen (Art. 21 DSGVO)",
      "Widerruf erteilter Einwilligungen mit Wirkung für die Zukunft",
      "Beschwerderecht bei einer Datenschutzaufsichtsbehörde",
      "Kontakt zur Ausübung: contact@tqdm.org"
    ],
    pointsEn: [
      "Access, rectification, erasure, restriction, portability (Art. 15–20 GDPR)",
      "Objection to certain processing (Art. 21 GDPR)",
      "Withdraw consent with future effect",
      "Right to lodge a complaint with a data protection authority",
      "Contact to exercise rights: contact@tqdm.org"
    ]
  },
  {
    id: "security",
    titleDe: "Sicherheit",
    titleEn: "Security",
    pointsDe: [
      "Angemessene technische und organisatorische Maßnahmen zum Schutz vor Verlust, Missbrauch und unbefugtem Zugriff.",
      "Bitte schützen Sie Ihre E-Mail-Inbox und Geräte, da der Login per Link erfolgt."
    ],
    pointsEn: [
      "Appropriate technical and organizational measures protect against loss, misuse, and unauthorized access.",
      "Please secure your email inbox and devices since login links are sent via email."
    ]
  },
  {
    id: "changes",
    titleDe: "Änderungen",
    titleEn: "Changes",
    pointsDe: [
      "Diese Datenschutzerklärung kann bei Bedarf aktualisiert werden. Die jeweils aktuelle Fassung finden Sie hier."
    ],
    pointsEn: [
      "This privacy notice may be updated as needed. The current version is published here."
    ]
  },
  {
    id: "contact",
    titleDe: "Kontakt",
    titleEn: "Contact",
    pointsDe: [
      "TQDM Inc., 1111B S Governors Ave, STE 23256, Dover, DE 19904, USA",
      "E-Mail: contact@tqdm.org | Telefon: +1 (814) 524-5685"
    ],
    pointsEn: [
      "TQDM Inc., 1111B S Governors Ave, STE 23256, Dover, DE 19904, USA",
      "Email: contact@tqdm.org | Phone: +1 (814) 524-5685"
    ]
  }
];

export const metadata: Metadata = {
  title: "Datenschutzerklärung / Privacy | dokustatus",
  description:
    "Datenschutzhinweise für den dokustatus-Dienst von TQDM Inc. (Minimalfassung ohne Tracking oder Werbung)."
};

export default async function PrivacyPage({
  params
}: {
  params: Promise<{ lang: "en" | "de" }>;
}) {
  await params;

  return (
    <main>
      <h1>Datenschutzerklärung / Privacy Policy</h1>
      <p className="text-muted">
        Minimalfassung ohne Tracking und Werbung. Der Dienst wird von TQDM Inc. in den USA betrieben;
        maßgeblich ist die deutschsprachige Fassung. / Minimal version without tracking or ads. The
        service is operated by TQDM Inc. in the USA; the German text prevails.
      </p>
      <div style={{ overflowX: "auto" }}>
        <table>
          <thead>
            <tr>
              <th style={{ width: "50%" }}>Deutsch</th>
              <th style={{ width: "50%" }}>English</th>
            </tr>
          </thead>
          <tbody>
            {sections.map((section) => (
              <tr key={section.id}>
                <td>
                  <h3 style={{ marginBottom: "8px" }}>{section.titleDe}</h3>
                  <ul style={{ paddingLeft: "18px", marginBottom: 0 }}>
                    {section.pointsDe.map((point, idx) => (
                      <li key={idx} style={{ marginBottom: "8px" }}>
                        {point}
                      </li>
                    ))}
                  </ul>
                </td>
                <td>
                  <h3 style={{ marginBottom: "8px" }}>{section.titleEn}</h3>
                  <ul style={{ paddingLeft: "18px", marginBottom: 0 }}>
                    {section.pointsEn.map((point, idx) => (
                      <li key={idx} style={{ marginBottom: "8px" }}>
                        {point}
                      </li>
                    ))}
                  </ul>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="text-muted" style={{ marginTop: "var(--spacing-md)" }}>
        Stand / Last updated: November 2025
      </p>
    </main>
  );
}
