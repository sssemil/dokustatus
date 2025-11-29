import type { Metadata } from "next";

type FooterLabels = {
  note: string;
  terms: string;
  privacy: string;
  imprint: string;
};

type Section = {
  id: string;
  titleDe: string;
  titleEn: string;
  pointsDe: string[];
  pointsEn: string[];
};

const sections: Section[] = [
  {
    id: "provider",
    titleDe: "Diensteanbieter",
    titleEn: "Service provider",
    pointsDe: [
      "TQDM Inc.",
      "1111B S Governors Ave, STE 23256, Dover, DE 19904, USA",
      "E-Mail: contact@tqdm.org | Telefon: +1 (814) 524-5685"
    ],
    pointsEn: [
      "TQDM Inc.",
      "1111B S Governors Ave, STE 23256, Dover, DE 19904, USA",
      "Email: contact@tqdm.org | Phone: +1 (814) 524-5685"
    ]
  },
  {
    id: "representation",
    titleDe: "Vertretungsberechtigte Person",
    titleEn: "Authorized representative",
    pointsDe: ["Emil Suleymanov, vertretungsberechtigt für TQDM Inc."],
    pointsEn: ["Emil Suleymanov, authorized to represent TQDM Inc."]
  },
  {
    id: "responsibility",
    titleDe: "Verantwortlich für den Inhalt (§ 18 Abs. 2 MStV)",
    titleEn: "Content responsibility (Sec. 18(2) MStV)",
    pointsDe: [
      "Emil Suleymanov",
      "1111B S Governors Ave, STE 23256, Dover, DE 19904, USA"
    ],
    pointsEn: [
      "Emil Suleymanov",
      "1111B S Governors Ave, STE 23256, Dover, DE 19904, USA"
    ]
  },
  {
    id: "register",
    titleDe: "Registereintrag",
    titleEn: "Register entry",
    pointsDe: [
      "Eingetragen nach dem Recht des US-Bundesstaats Delaware.",
      "State of Delaware — File Number: bitte ergänzen (falls gewünscht)."
    ],
    pointsEn: [
      "Incorporated under the laws of the State of Delaware, USA.",
      "State of Delaware — File Number: not publicly listed here."
    ]
  },
  {
    id: "tax",
    titleDe: "Umsatzsteuer",
    titleEn: "VAT",
    pointsDe: [
      "Keine deutsche USt-IdNr. vorhanden (US-Kapitalgesellschaft, sofern keine steuerpflichtigen Umsätze in Deutschland)."
    ],
    pointsEn: [
      "No German VAT ID (US corporation, assuming no taxable supplies in Germany)."
    ]
  },
  {
    id: "settlement",
    titleDe: "Online-Streitbeilegung / Verbraucherschlichtung",
    titleEn: "Online dispute resolution / consumer arbitration",
    pointsDe: [
      "EU-OS-Plattform: https://ec.europa.eu/consumers/odr/",
      "Wir sind nicht verpflichtet und grundsätzlich nicht bereit, an Streitbeilegungsverfahren vor einer Verbraucherschlichtungsstelle teilzunehmen."
    ],
    pointsEn: [
      "EU ODR platform: https://ec.europa.eu/consumers/odr/",
      "We are not obliged and generally not willing to participate in consumer arbitration proceedings."
    ]
  },
  {
    id: "liability",
    titleDe: "Haftungshinweise",
    titleEn: "Liability notes",
    pointsDe: [
      "Für Inhalte externer Links übernehmen wir trotz sorgfältiger Kontrolle keine Haftung; hierfür sind ausschließlich deren Betreiber verantwortlich.",
      "Eigene Inhalte werden nach bestem Wissen gepflegt; maßgeblich bleiben die Nutzungsbedingungen."
    ],
    pointsEn: [
      "We accept no liability for external links despite careful checks; the operators of the linked sites are solely responsible.",
      "Own content is maintained to the best of our knowledge; the terms of service remain decisive."
    ]
  },
  {
    id: "updates",
    titleDe: "Änderungen",
    titleEn: "Changes",
    pointsDe: [
      "Bitte halten Sie diese Angaben aktuell und passen Sie das Impressum bei Änderungen unverzüglich an."
    ],
    pointsEn: [
      "Keep these details current and update the imprint promptly when changes occur."
    ]
  }
];

const footerLabels: Record<"en" | "de", FooterLabels> = {
  en: {
    note: "This service is provided without guarantee; see terms for details.",
    terms: "Terms & Conditions",
    privacy: "Privacy Policy",
    imprint: "Imprint"
  },
  de: {
    note: "Dienst ohne Gewähr; Details in den Nutzungsbedingungen.",
    terms: "Nutzungsbedingungen",
    privacy: "Datenschutzerklärung",
    imprint: "Impressum"
  }
};

export const metadata: Metadata = {
  title: "Impressum | dokustatus",
  description:
    "Impressum / legal notice for the dokustatus document tracking service by TQDM Inc."
};

export default async function ImpressumPage({
  params
}: {
  params: Promise<{ lang: "en" | "de" }>;
}) {
  const { lang } = await params;
  const labels = footerLabels[lang];

  return (
    <div style={{ display: "flex", minHeight: "100vh", flexDirection: "column" }}>
      <main style={{ flex: 1 }}>
        <h1>Impressum / Legal Notice</h1>
        <p className="text-muted">
          Angaben nach § 5 TMG und § 18 Abs. 2 MStV für TQDM Inc. / Legal notice for TQDM Inc.
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
          Stand / Last updated: November 2024
        </p>
      </main>

      <footer
        style={{
          borderTop: "1px solid var(--border-primary)",
          background: "var(--bg-secondary)",
          padding: "var(--spacing-md) var(--spacing-xl)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          color: "var(--text-muted)",
          fontSize: "12px"
        }}
      >
        <span>{labels.note}</span>
        <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
          <a href={`/${lang}/terms`} style={{ color: "var(--text-link)", textDecoration: "none", fontSize: "13px" }}>
            {labels.terms}
          </a>
          <a href={`/${lang}/privacy`} style={{ color: "var(--text-link)", textDecoration: "none", fontSize: "13px" }}>
            {labels.privacy}
          </a>
          <a href={`/${lang}/impressum`} style={{ color: "var(--text-link)", textDecoration: "none", fontSize: "13px" }}>
            {labels.imprint}
          </a>
        </div>
      </footer>
    </div>
  );
}
